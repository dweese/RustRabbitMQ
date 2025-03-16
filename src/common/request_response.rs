use futures::stream::StreamExt;
use lapin::{
    message::Delivery, options::*, types::FieldTable, BasicProperties, Channel, Connection,
    Error as LapinError, ExchangeKind,
}; // from futures

use futures::TryStreamExt;
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::oneshot;
use tracing::{error, info};
use uuid::Uuid;

use super::connection::ConnectionManager; // Access the ConnectionManager from connection.rs module

#[derive(Error, Debug)]
pub enum RpcError {
    #[error("Failed to connect to RabbitMQ: {0}")]
    ConnectionError(#[from] LapinError),

    #[error("Failed to serialize/deserialize message: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Request timeout after {0:?}")]
    Timeout(Duration),

    #[error("Response channel closed")]
    ResponseChannelClosed,
}

#[derive(Debug, Serialize, Deserialize)]
struct InventoryRequest {
    product_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct InventoryResponse {
    product_id: String,
    quantity: i32,
    available: bool,
}

type ResponseCallback = oneshot::Sender<Result<Vec<u8>, RpcError>>;

struct RpcClient {
    connection_manager: ConnectionManager,
    channel: Option<Channel>,
    response_queue: String,
    correlation_map: Arc<Mutex<HashMap<String, ResponseCallback>>>,
    timeout: Duration,
}

impl RpcClient {
    pub async fn new(uri: &str, timeout_secs: u64) -> Result<Self, RpcError> {
        let connection_manager = ConnectionManager::new(uri).with_reconnect_policy(5, 1000);

        let response_queue = format!("rpc.response.{}", Uuid::new_v4());

        let client = RpcClient {
            connection_manager,
            channel: None,
            response_queue,
            correlation_map: Arc::new(Mutex::new(HashMap::new())),
            timeout: Duration::from_secs(timeout_secs),
        };

        Ok(client)
    }

    async fn setup(&mut self) -> Result<&Channel, RpcError> {
        if let Some(channel) = &self.channel {
            if channel.status().connected() {
                return Ok(channel);
            }
        }

        let connection = self.connection_manager.get_connection().await?;
        let channel = connection
            .create_channel()
            .await
            .map_err(|e| RpcError::ChannelError(e.to_string()))?;

        // Declare response queue (exclusive, auto-delete)
        channel
            .queue_declare(
                &self.response_queue,
                QueueDeclareOptions {
                    exclusive: true,
                    auto_delete: true,
                    ..QueueDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                RpcError::ChannelError(format!("Failed to declare response queue: {}", e))
            })?;

        // Setup consumer for response queue
        let consumer = channel
            .basic_consume(
                &self.response_queue,
                "rpc_client",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| RpcError::ChannelError(format!("Failed to create consumer: {}", e)))?;

        let correlation_map = self.correlation_map.clone();

        // Process responses
        let mut consumer_stream = consumer.into_stream();

        tokio::spawn(async move {
            while let Some(delivery_result) = consumer_stream.next().await {
                match delivery_result {
                    Ok(delivery) => {
                        if let Some(correlation_id) = delivery.properties.correlation_id() {
                            let correlation_id = correlation_id.as_str().to_string();

                            // Find the corresponding callback
                            let mut map = correlation_map.lock().unwrap();
                            if let Some(callback) = map.remove(&correlation_id) {
                                // Acknowledge the message
                                if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                    error!("Failed to acknowledge response: {}", e);
                                }

                                // Send the response back
                                let _ = callback.send(Ok(delivery.data));
                            } else {
                                error!(
                                    "Received response with unknown correlation ID: {}",
                                    correlation_id
                                );

                                // Reject the message
                                if let Err(e) = delivery.reject(BasicRejectOptions::default()).await
                                {
                                    error!("Failed to reject message: {}", e);
                                }
                            }
                        } else {
                            error!("Received response without correlation ID");

                            // Reject the message
                            if let Err(e) = delivery.reject(BasicRejectOptions::default()).await {
                                error!("Failed to reject message: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error receiving response: {}", e);
                    }
                }
            }
        });

        self.channel = Some(channel);
        Ok(self.channel.as_ref().unwrap())
    }

    pub async fn call<T: Serialize, R: for<'de> Deserialize<'de>>(
        &mut self,
        exchange: &str,
        routing_key: &str,
        request: &T,
    ) -> Result<R, RpcError> {
        let channel = self.setup().await?;

        // Generate correlation ID
        let correlation_id = Uuid::new_v4().to_string();

        // Serialize request
        let payload = serde_json::to_vec(request)?;

        // Create response channel
        let (tx, rx) = oneshot::channel();

        // Store callback
        {
            let mut map = self.correlation_map.lock().unwrap();
            map.insert(correlation_id.clone(), tx);
        }

        // Publish request
        let properties = BasicProperties::default()
            .with_correlation_id(correlation_id.clone().into())
            .with_reply_to(self.response_queue.clone().into())
            .with_content_type("application/json".into());

        channel
            .basic_publish(
                exchange,
                routing_key,
                BasicPublishOptions::default(),
                &payload,
                properties,
            )
            .await
            .map_err(|e| RpcError::ChannelError(format!("Failed to publish request: {}", e)))?;

        // Wait for response with timeout
        let response_data = tokio::time::timeout(self.timeout, rx)
            .await
            .map_err(|_| RpcError::Timeout(self.timeout))?
            .map_err(|_| RpcError::ResponseChannelClosed)??;

        // Deserialize response
        let response: R = serde_json::from_slice(&response_data)?;
        Ok(response)
    }

    pub async fn close(&mut self) -> Result<(), RpcError> {
        if let Some(channel) = &self.channel {
            channel
                .close(0, "Closing RPC client")
                .await
                .map_err(|e| RpcError::ChannelError(e.to_string()))?;
        }

        self.connection_manager.close().await?;
        Ok(())
    }
}

// RPC Server to handle inventory requests
struct RpcServer {
    connection_manager: ConnectionManager,
    channel: Option<Channel>,
    queue: String,
}

impl RpcServer {
    pub async fn new(uri: &str, queue: &str) -> Result<Self, RpcError> {
        let connection_manager = ConnectionManager::new(uri).with_reconnect_policy(5, 1000);

        Ok(RpcServer {
            connection_manager,
            channel: None,
            queue: queue.to_string(),
        })
    }

    async fn setup(&mut self) -> Result<&Channel, RpcError> {
        if let Some(channel) = &self.channel {
            if channel.status().is_connected() {
                return Ok(channel);
            }
        }

        let connection = self.connection_manager.get_connection().await?;
        let channel = connection
            .create_channel()
            .await
            .map_err(|e| RpcError::ChannelError(e.to_string()))?;

        // Declare queue
        channel
            .queue_declare(
                &self.queue,
                QueueDeclareOptions {
                    durable: true,
                    ..QueueDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| RpcError::ChannelError(format!("Failed to declare queue: {}", e)))?;

        self.channel = Some(channel);
        Ok(self.channel.as_ref().unwrap())
    }

    pub async fn start<F, Fut>(&mut self, handler: F) -> Result<(), RpcError>
    where
        F: Fn(InventoryRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<InventoryResponse, RpcError>> + Send + 'static,
    {
        let channel = self.setup().await?;

        // Set prefetch count
        channel
            .basic_qos(1, BasicQosOptions::default())
            .await
            .map_err(|e| RpcError::ChannelError(format!("Failed to set QoS: {}", e)))?;

        // Start consuming requests
        let consumer = channel
            .basic_consume(
                &self.queue,
                "rpc_server",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| RpcError::ChannelError(format!("Failed to create consumer: {}", e)))?;

        info!("RPC server started on queue: {}", self.queue);

        let mut consumer_stream = consumer.into_stream();
        let channel_ref = channel.clone();

        // Handle requests
        while let Some(delivery_result) = consumer_stream.next().await {
            match delivery_result {
                Ok(delivery) => {
                    let channel = channel_ref.clone();

                    // Process request in a separate task
                    tokio::spawn(async move {
                        Self::process_request(channel, delivery, handler.clone()).await;
                    });
                }
                Err(e) => {
                    error!("Error receiving request: {}", e);

                    // Check if we need t
                    // o reconnect
                    if !channel.status().connected() {
                        warn!("Channel disconnected, attempting to reconnect");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    async fn process_request<F, Fut>(channel: Channel, delivery: Delivery, handler: F)
    where
        F: Fn(InventoryRequest) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<InventoryResponse, RpcError>> + Send,
    {
        // Extract reply_to and correlation_id
        let reply_to = match delivery.properties.reply_to() {
            Some(reply_to) => reply_to.as_str(),
            None => {
                error!("Received RPC request without reply_to");
                let _ = delivery.reject(BasicRejectOptions::default()).await;
                return;
            }
        };

        let correlation_id = match delivery.properties.correlation_id() {
            Some(correlation_id) => correlation_id.as_str().to_string(),
            None => {
                error!("Received RPC request without correlation_id");
                let _ = delivery.reject(BasicRejectOptions::default()).await;
                return;
            }
        };

        // Deserialize request
        let request: InventoryRequest = match serde_json::from_slice(&delivery.data) {
            Ok(request) => request,
            Err(e) => {
                error!("Failed to deserialize request: {}", e);
                let _ = delivery.reject(BasicRejectOptions::default()).await;
                return;
            }
        };

        info!(
            "Received inventory request for product: {}",
            request.product_id
        );

        // Process the request
        let response_result = handler(request).await;

        // Send response
        match response_result {
            Ok(response) => match serde_json::to_vec(&response) {
                Ok(payload) => {
                    let properties = BasicProperties::default()
                        .with_correlation_id(correlation_id.into())
                        .with_content_type("application/json".into());

                    match channel
                        .basic_publish(
                            "",
                            reply_to,
                            BasicPublishOptions::default(),
                            &payload,
                            properties,
                        )
                        .await
                    {
                        Ok(_) => {
                            info!(
                                "Sent inventory response for product: {}",
                                response.product_id
                            );
                            let _ = delivery.ack(BasicAckOptions::default()).await;
                        }

                        Err(e) => {
                            error!("Failed to send response: {}", e);
                            let _ = delivery
                                .reject(BasicRejectOptions {
                                    requeue: true,
                                    ..BasicRejectOptions::default()
                                })
                                .await;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to serialize response: {}", e);
                    let _ = delivery.reject(BasicRejectOptions::default()).await;
                }
            },
            Err(e) => {
                error!("Error processing request: {}", e);
                let _ = delivery.reject(BasicRejectOptions::default()).await;
            }
        }
    }

    pub async fn close(&mut self) -> Result<(), RpcError> {
        if let Some(channel) = &self.channel {
            channel
                .close(0, "Closing RPC server")
                .await
                .map_err(|e| RpcError::ChannelError(e.to_string()))?;
        }

        self.connection_manager.close().await?;
        Ok(())
    }
}

// Example usage
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup tracing
    tracing_subscriber::fmt::init();

    // Load configuration from environment or use defaults
    dotenv::dotenv().ok();
    let rabbitmq_uri = std::env::var("RABBITMQ_URI")
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/%2f".to_string());

    // Mock inventory database (product_id -> quantity)
    let inventory = Arc::new(Mutex::new(HashMap::from([
        ("product-1".to_string(), 10),
        ("product-2".to_string(), 5),
        ("product-3".to_string(), 0),
    ])));

    // Start RPC server in a separate task
    let server_uri = rabbitmq_uri.clone();
    let inventory_clone = inventory.clone();
    tokio::spawn(async move {
        let mut server = RpcServer::new(&server_uri, "inventory_requests")
            .await
            .unwrap();

        server
            .start(move |request: InventoryRequest| {
                let inventory = inventory_clone.clone();
                async move {
                    // Simulate processing delay
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    let quantity = {
                        let inv = inventory.lock().unwrap();
                        *inv.get(&request.product_id).unwrap_or(&0)
                    };

                    Ok(InventoryResponse {
                        product_id: request.product_id,
                        quantity,
                        available: quantity > 0,
                    })
                }
            })
            .await
            .unwrap();
    });

    // Wait for the server to start
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Create RPC client
    let mut client = RpcClient::new(&rabbitmq_uri, 5).await?;

    // Send inventory requests
    for product_id in ["product-1", "product-2", "product-3", "product-4"] {
        let request = InventoryRequest {
            product_id: product_id.to_string(),
        };

        match client
            .call::<_, InventoryResponse>("", "inventory_requests", &request)
            .await
        {
            Ok(response) => {
                println!(
                    "Product {} availability: {}",
                    response.product_id,
                    if response.available {
                        "In stock"
                    } else {
                        "Out of stock"
                    }
                );
                println!("Quantity: {}", response.quantity);
            }
            Err(e) => {
                println!("Failed to get inventory for {}: {}", product_id, e);
            }
        }
    }

    // Close client
    client.close().await?;

    Ok(())
}
