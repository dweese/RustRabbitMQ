#![allow(dead_code)]

use futures::stream::StreamExt;
use lapin::{message::Delivery, options::*, types::FieldTable, BasicProperties, Channel}; // from futures

use futures::TryStreamExt;
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::connection::ConnectionManager; // Access the ConnectionManager from connection.rs module
use tokio::sync::oneshot::{self, Sender};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Exchange error: {0}")]
    ExchangeError(String),

    #[error("Queue error: {0}")]
    QueueError(String),

    #[error("Publish error: {0}")]
    PublishError(String), // Add this variant

    #[error("Consumer error: {0}")]
    ConsumerError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Timeout error: {0}")]
    TimeoutError(String),

    #[error("RPC request timed out after {0:?}")]
    Timeout(Duration),

    #[error("Response channel was closed unexpectedly")]
    ResponseChannelClosed,

    #[error("Unknown error: {0}")]
    UnknownError(String),
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
    response_channels: Arc<Mutex<HashMap<String, Sender<Vec<u8>>>>>,
    exchange: String, // Add this field
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
            response_channels: Arc::new(Mutex::new(HashMap::new())),
            exchange: String::from("amq.topic"),
        };

        Ok(client)
    }

    async fn setup(&mut self, exchange: &str) -> Result<(), RpcError> {
        // First check if we have a valid channel
        if let Some(channel) = &self.channel {
            if channel.status().connected() {
                return Ok(()); // Channel is valid, no need to create a new one
            }
        }

        // If we don't have a valid channel, create a new one
        let connection = self
            .connection_manager
            .get_connection()
            .await
            .map_err(|e| RpcError::ChannelError(format!("{:?}", e)))?;

        let channel = connection
            .create_channel()
            .await
            .map_err(|e| RpcError::ChannelError(e.to_string()))?;

        // Setup exchange
        channel
            .exchange_declare(
                exchange,
                lapin::ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..ExchangeDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| RpcError::ExchangeError(e.to_string()))?;

        // Directly assign the channel
        self.channel = Some(channel);

        Ok(())
    }

    // And add a new method to get the channel
    fn get_channel(&self) -> Result<&Channel, RpcError> {
        self.channel
            .as_ref()
            .ok_or_else(|| RpcError::ChannelError("No channel available".to_string()))
    }

    pub async fn call<T: Serialize, R: for<'de> Deserialize<'de>>(
        &mut self,
        exchange: &str,
        routing_key: &str,
        request: &T,
    ) -> Result<R, RpcError> {
        // First ensure we have a valid channel
        self.ensure_channel(exchange).await?;

        // Now all operations that use the channel directly inside this method
        let correlation_id = Uuid::new_v4().to_string();
        let reply_to = format!("amq.rabbitmq.reply-to");

        // Create a oneshot channel for receiving the response
        let (tx, rx) = tokio::sync::oneshot::channel();
        // Store the sender in a map keyed by correlation_id
        self.response_channels
            .lock()
            .unwrap()
            .insert(correlation_id.clone(), tx);

        // Serialize the request
        let payload =
            serde_json::to_vec(request).map_err(|e| RpcError::SerializationError(e.to_string()))?;

        // This is safe because we just ensured we have a valid channel
        let channel = self.channel.as_ref().unwrap();

        // Publish
        channel
            .basic_publish(
                exchange,
                routing_key,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default()
                    .with_correlation_id(correlation_id.clone().into())
                    .with_reply_to(reply_to.clone().into()),
            )
            .await
            .map_err(|e| RpcError::PublishError(e.to_string()))?;

        // Wait for response with timeout
        let response_data = tokio::time::timeout(self.timeout, rx)
            .await
            .map_err(|_| RpcError::Timeout(self.timeout))? // Handles the timeout Result
            .map_err(|_| RpcError::ResponseChannelClosed)?; // Handles the oneshot channel Result

        // Deserialize and return
        let response: R = serde_json::from_slice(&response_data)
            .map_err(|e| RpcError::DeserializationError(e.to_string()))?;

        Ok(response)
    }

    pub async fn close(&mut self) -> Result<(), RpcError> {
        if let Some(channel) = &self.channel {
            channel
                .close(0, "Closing RPC client")
                .await
                .map_err(|e| RpcError::ChannelError(e.to_string()))?;
        }

        // Convert the connection manager's error to RpcError
        self.connection_manager
            .close()
            .await
            .map_err(|e| RpcError::ConnectionError(e.to_string()))?;

        Ok(())
    }

    async fn ensure_channel(&mut self, exchange: &str) -> Result<(), RpcError> {
        self.setup(exchange).await
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
        // Check if we need a new channel without early returns
        let need_new_channel = match &self.channel {
            Some(channel) if channel.status().connected() => false,
            _ => true,
        };

        // Create a new channel if needed
        if need_new_channel {
            // Get connection
            let connection = self
                .connection_manager
                .get_connection()
                .await
                .map_err(|e| RpcError::ConnectionError(e.to_string()))?;

            // Create channel
            let channel = connection
                .create_channel()
                .await
                .map_err(|e| RpcError::ChannelError(format!("Failed to create channel: {}", e)))?;

            // Declare queue before storing the channel
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

            // Store the new channel
            self.channel = Some(channel);
        }

        // Now we can safely return a reference to self.channel, which is guaranteed to exist
        match &self.channel {
            Some(channel) => Ok(channel),
            None => Err(RpcError::ChannelError(
                "Channel creation failed".to_string(),
            )),
        }
    }

    // Simply replace ensure_channel with your existing setup method
    async fn ensure_channel(&mut self) -> Result<&Channel, RpcError> {
        self.setup().await
    }

    fn get_channel(&self) -> Result<&Channel, RpcError> {
        match &self.channel {
            Some(channel) => Ok(channel),
            None => Err(RpcError::ChannelError("No channel available".to_string())),
        }
    }

    pub async fn start<F, Fut>(&mut self, handler: F) -> Result<(), RpcError>
    where
        F: Fn(InventoryRequest) -> Fut + Send + Sync + 'static, // Removed Clone
        Fut: std::future::Future<Output = Result<InventoryResponse, RpcError>> + Send + 'static,
    {
        self.setup().await?;
        let channel = self.get_channel()?;

        // Put handler in Arc before the loop
        let handler = std::sync::Arc::new(handler);

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
                    let handler = handler.clone(); // Cloning the Arc, not the handler itself

                    tokio::spawn(async move {
                        if let Err(e) = Self::process_request(channel, delivery, handler).await {
                            error!("CRITICAL ERROR in RPC request processing: {:?}", e);
                            // You could add additional error handling here if needed
                            // For example, metrics increment or alerting
                        }
                    });
                }
                Err(e) => {
                    error!("Error receiving request: {}", e);

                    // Check if we need to reconnect
                    if !channel.status().connected() {
                        warn!("Channel disconnected, attempting to reconnect");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    async fn process_request<F, Fut>(
        channel: Channel,
        delivery: Delivery,
        handler: Arc<F>,
    ) -> Result<(), RpcError>
    where
        F: Fn(InventoryRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<InventoryResponse, RpcError>> + Send + 'static,
    {
        // Extract reply_to and correlation_id
        let reply_to = match delivery.properties.reply_to() {
            Some(reply_to) => reply_to.as_str(),
            None => {
                error!("Received RPC request without reply_to");
                let _ = delivery.reject(BasicRejectOptions::default()).await;
                return Ok(());
            }
        };

        let correlation_id = match delivery.properties.correlation_id() {
            Some(correlation_id) => correlation_id.as_str().to_string(),
            None => {
                error!("Received RPC request without correlation_id");
                let _ = delivery.reject(BasicRejectOptions::default()).await;
                return Ok(());
            }
        };

        // Deserialize request
        let request: InventoryRequest = match serde_json::from_slice(&delivery.data) {
            Ok(request) => request,
            Err(e) => {
                error!("Failed to deserialize request: {}", e);
                let _ = delivery.reject(BasicRejectOptions::default()).await;
                return Ok(());
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
                            Ok(()) // Add this line to return the expected Result type
                        }

                        Err(e) => {
                            error!("Failed to send response: {}", e);
                            let _ = delivery
                                .reject(BasicRejectOptions {
                                    requeue: true,
                                    ..BasicRejectOptions::default()
                                })
                                .await;
                            Ok(()) // Add this line to return the expected Result type
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to serialize response: {}", e);
                    let _ = delivery.reject(BasicRejectOptions::default()).await;
                    Ok(()) // Add this line to return the expected Result type
                }
            },
            Err(e) => {
                error!("Error processing request: {}", e);
                let _ = delivery.reject(BasicRejectOptions::default()).await;
                Ok(()) // Add this line to return the expected Result type
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

        // Explicitly map the error to RpcError
        self.connection_manager
            .close()
            .await
            .map_err(|e| RpcError::ConnectionError(format!("Failed to close connection: {}", e)))?;

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
        let mut server = match RpcServer::new(&server_uri, "inventory_requests").await {
            Ok(server) => server,
            Err(e) => {
                tracing::error!("Failed to create RPC server: {}", e);
                return; // Exit the async task
            }
        };

        if let Err(e) = server
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
        {
            tracing::error!("Failed to start RPC server handler: {}", e);
            return;
        }
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
                tracing::info!(
                    product_id = %response.product_id,
                    available = response.available,
                    quantity = response.quantity,
                    "Inventory check result"
                );
            }
            Err(e) => {
                tracing::error!(
                    product_id = %product_id,
                    error = %e,
                    "Failed to get inventory"
                );
            }
        }
    }

    // Close client
    client.close().await?;

    Ok(())
}
