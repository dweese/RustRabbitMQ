use futures::TryStreamExt;
use futures_lite::StreamExt; // Add this import

use lapin::{
    options::*, types::FieldTable, Channel, Connection, Consumer, Error as LapinError, ExchangeKind,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{error, info, warn};

use super::connection::ConnectionManager;  // Access the ConnectionManager from connection.rs module


#[derive(Error, Debug)]
pub enum ConsumerError {
    #[error("Failed to connect to RabbitMQ: {0}")]
    ConnectionError(#[from] LapinError),

    #[error("Failed to deserialize message: {0}")]
    DeserializationError(#[from] serde_json::Error),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Consumer error: {0}")]
    ConsumerError(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct OrderMessage {
    order_id: String,
    customer_id: String,
    items: Vec<String>,
    total: f64,
    timestamp: chrono::DateTime<chrono::Utc>,
}

struct MessageConsumer {
    connection_manager: ConnectionManager,
    channel: Option<Channel>,
    exchange: String,
    queue: String,
}

impl MessageConsumer {
    pub async fn new(uri: &str, exchange: &str, queue: &str) -> Result<Self, ConsumerError> {
        let connection_manager = ConnectionManager::new(uri).with_reconnect_policy(5, 1000);

        Ok(MessageConsumer {
            connection_manager,
            channel: None,
            exchange: exchange.to_string(),
            queue: queue.to_string(),
        })
    }

    async fn setup_channel(&mut self) -> Result<&Channel, ConsumerError> {
        if let Some(channel) = &self.channel {
            if channel.status().connected() {
                return Ok(channel);
            }
        }

        let connection = self.connection_manager.get_connection().await?;
        let channel = connection
            .create_channel()
            .await
            .map_err(|e| ConsumerError::ChannelError(e.to_string()))?;

        // Declare the exchange
        channel
            .exchange_declare(
                &self.exchange,
                ExchangeKind::Direct,
                ExchangeDeclareOptions {
                    durable: true,
                    ..ExchangeDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                ConsumerError::ChannelError(format!("Failed to declare exchange: {}", e))
            })?;


        info!("About to declare queue: {}", self.queue);

        // Declare the queue
        // Make sure this runs before attempting to consume
        let queue_declare_result = channel
            .queue_declare(
                &self.queue,
                QueueDeclareOptions {
                    durable: true,
                    ..QueueDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| ConsumerError::ChannelError(format!("Failed to declare queue: {}", e)))?;

        info!("Queue '{}' declared successfully: {:?}", self.queue, queue_declare_result);

        // Bind the queue to the exchange
        channel
            .queue_bind(
                &self.queue,
                &self.exchange,
                "user_registered", // Routing key
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| ConsumerError::ChannelError(format!("Failed to bind queue: {}", e)))?;

        self.channel = Some(channel);


        Ok(self.channel.as_ref().unwrap())
    }

    pub async fn start_consuming<F>(&mut self, handler: F) -> Result<(), ConsumerError>
    where
        F: Fn(OrderMessage) -> Result<(), Box<dyn std::error::Error>> + Send + Sync + 'static,
    {
        let channel = self.setup_channel().await?;
        let consumer = channel
            .basic_consume(
                &self.queue,
                &format!("consumer-{}", uuid::Uuid::new_v4()),  // Use only this as the consumer tag
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await.map_err(|e| ConsumerError::ConsumerError(e.to_string()))?;

        info!("Started consuming from queue: {}", self.queue);

        let handler = std::sync::Arc::new(handler);

        // Process messages
        let mut consumer_stream = consumer.into_stream();

        while let Some(delivery_result) = consumer_stream.next().await {
            match delivery_result {
                Ok(delivery) => {
                    let handler = handler.clone();

                    // Process the message in a separate task
                    tokio::spawn(async move {
                        match serde_json::from_slice::<OrderMessage>(&delivery.data) {
                            Ok(order) => {
                                info!("Received order: {:?}", order);

                                match handler(order) {
                                    Ok(_) => {
                                        // Acknowledge the message
                                        if let Err(e) =
                                            delivery.ack(BasicAckOptions::default()).await
                                        {
                                            error!("Failed to acknowledge message: {}", e);
                                        }
                                    }
                                    Err(e) => {
                                        error!("Error processing message: {}", e);
                                        // Negative acknowledge the message
                                        if let Err(e) = delivery
                                            .nack(BasicNackOptions {
                                                requeue: true,
                                                ..BasicNackOptions::default()
                                            })
                                            .await
                                        {
                                            error!(
                                                "Failed to negatively acknowledge message: {}",
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to deserialize message: {}", e);
                                // Reject malformed messages
                                if let Err(e) =
                                    delivery.reject(BasicRejectOptions { requeue: false }).await
                                {
                                    error!("Failed to reject message: {}", e);
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("Error receiving message: {}", e);

                    // Check if we need to reconnect or just continue
                    if !channel.status().connected() {
                        warn!("Channel disconnected, attempting to reconnect");
                        // The next iteration will try to reconnect
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn close(&mut self) -> Result<(), ConsumerError> {
        if let Some(channel) = &self.channel {
            channel
                .close(0, "Closing consumer")
                .await
                .map_err(|e| ConsumerError::ChannelError(e.to_string()))?;
        }

        self.connection_manager.close().await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup tracing
    tracing_subscriber::fmt::init();

    // Load configuration from environment or use defaults
    dotenv::dotenv().ok();
    let rabbitmq_uri = std::env::var("RABBITMQ_URI")
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/vhost_rust".to_string());

    // Create consumer
    let mut consumer = MessageConsumer::new(&rabbitmq_uri, "amq.direct", "user_registered").await?;

    // Explicitly ensure the queue exists before consuming
    consumer.setup_channel().await?;

    consumer
        .start_consuming(|order: OrderMessage| {
            println!(
                "Processing order {} for customer {}",
                order.order_id, order.customer_id
            );
            println!("Items: {:?}, Total: ${:.2}", order.items, order.total);

            // Simulate processing time
            std::thread::sleep(std::time::Duration::from_millis(500));

            println!("Order {} processed successfully!", order.order_id);
            Ok(())
        })
        .await?;

    // The consumer will continue running until the program is terminated

    Ok(())
}
