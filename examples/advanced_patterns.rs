// examples/advanced_patterns.rs
use futures::StreamExt;
use lapin::{
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicPublishOptions,
        BasicRejectOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
    },
    types::{FieldTable, LongString},
    Channel, Connection,
};
use lapin::message::Delivery;
use lapin::publisher_confirm::PublisherConfirm;
use lapin::BasicProperties;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;
use crate::rabbitmq::errors::{RabbitMQError, Result};

//
// // Define RabbitError
// #[derive(Debug, Error)]
// pub enum RabbitMQError {
//     #[error("Connection error: {0}")]
//     ConnectionError(String),
//     #[error("Channel error: {0}")]
//     ChannelError(String),
//     #[error("Consume error: {0}")]
//     ConsumeError(String),
//     #[error("Acknowledgement error: {0}")]
//     AckError(String),
//     #[error("JSON serialization error: {0}")]
//     SerializationError(#[from] serde_json::Error),
// }

// ConnectionManager for handling reconnections
struct ConnectionManager {
    uri: String,
    max_retries: usize,
    retry_interval: u64,
    connection: Option<Connection>,
}

impl ConnectionManager {
    pub fn new(uri: &str) -> Self {
        ConnectionManager {
            uri: uri.to_string(),
            max_retries: 3,
            retry_interval: 500,
            connection: None,
        }
    }

    pub fn with_reconnect_policy(mut self, max_retries: usize, retry_interval: u64) -> Self {
        self.max_retries = max_retries;
        self.retry_interval = retry_interval;
        self
    }

    pub async fn get_connection(&mut self) -> Result<&Connection, RabbitMQError> {
        if let Some(conn) = &self.connection {
            if conn.status().connected() {
                return Ok(conn);
            }
        }

        // Try to establish a new connection
        let mut retries = 0;
        let mut last_error = None;

        while retries < self.max_retries {
            match Connection::connect(&self.uri, Default::default()).await {
                Ok(conn) => {
                    self.connection = Some(conn);
                    return Ok(self.connection.as_ref().unwrap());
                }
                Err(err) => {
                    last_error = Some(err.to_string());
                    retries += 1;
                    tokio::time::sleep(Duration::from_millis(self.retry_interval)).await;
                }
            }
        }

        Err(RabbitMQError::ConnectionError(format!(
            "Failed to connect after {} attempts: {}",
            self.max_retries,
            last_error.unwrap_or_else(|| "Unknown error".to_string())
        )))
    }

    pub async fn close(&mut self) -> Result<(), RabbitMQError> {
        if let Some(conn) = &self.connection {
            conn.close(0, "Closing connection")
                .await
                .map_err(|e| RabbitMQError::ConnectionError(e.to_string()))?;
            self.connection = None;
        }
        Ok(())
    }
}

// Unified PaymentMessage definition
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PaymentMessage {
    payment_id: String,
    order_id: String,
    amount: f64,
    status: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

// Dead Letter Publisher
struct DeadLetterPublisher {
    connection_manager: ConnectionManager,
    channel: Option<Channel>,
    exchange: String,
    dead_letter_exchange: String,
    url: String,
}

impl DeadLetterPublisher {
    pub async fn new(
        uri: &str,
        exchange: &str,
        dead_letter_exchange: &str,
    ) -> Result<Self, RabbitMQError> {
        let connection_manager = ConnectionManager::new(uri).with_reconnect_policy(5, 1000);

        Ok(DeadLetterPublisher {
            connection_manager,
            channel: None,
            exchange: exchange.to_string(),
            dead_letter_exchange: dead_letter_exchange.to_string(),
            url: uri.to_string(),
        })
    }

    async fn setup_channel(&mut self) -> Result<Channel, RabbitMQError> {
        // Create a new channel
        let connection = self.connection_manager.get_connection().await?;
        let channel = connection
            .create_channel()
            .await
            .map_err(|e| RabbitMQError::ChannelError(e.to_string()))?;

        // Store it and return a clone
        self.channel = Some(channel.clone());
        Ok(channel)
    }

    pub async fn publish(&mut self, message: &PaymentMessage) -> Result<(), RabbitMQError> {
        let channel = self.setup_channel().await?;

        let payload = serde_json::to_vec(message)?;

        // Add message expiration (TTL) of 10 seconds
        let properties = BasicProperties::default()
            .with_message_id(Uuid::new_v4().to_string().into())
            .with_content_type("application/json".into())
            .with_expiration("10000".into()); // 10 seconds TTL

        channel
            .basic_publish(
                &self.exchange,
                "payment",
                BasicPublishOptions::default(),
                &payload,
                properties,
            )
            .await
            .map_err(|e| RabbitMQError::ChannelError(format!("Failed to publish message: {}", e)))?;

        info!("Published payment message: {}", message.payment_id);

        Ok(())
    }

    pub async fn close(&mut self) -> Result<(), RabbitMQError> {
        if let Some(channel) = &self.channel {
            channel
                .close(0, "Closing publisher")
                .await
                .map_err(|e| RabbitMQError::ChannelError(e.to_string()))?;
        }

        self.connection_manager.close().await?;
        Ok(())
    }
}

// Batch Processing Consumer
struct BatchConsumer {
    connection_manager: ConnectionManager,
    channel: Option<Channel>,
    exchange: String, // Added exchange field
    queue: String,
    batch_size: usize,
    batch_timeout: Duration,
}

impl BatchConsumer {
    pub async fn new(
        uri: &str,
        exchange: &str, // Added exchange parameter
        queue: &str,
        batch_size: usize,
        batch_timeout_secs: u64, // Changed to seconds for clarity
    ) -> Result<Self, RabbitMQError> {
        let connection_manager = ConnectionManager::new(uri).with_reconnect_policy(5, 1000);

        Ok(BatchConsumer {
            connection_manager,
            channel: None,
            exchange: exchange.to_string(), // Store exchange
            queue: queue.to_string(),
            batch_size,
            batch_timeout: Duration::from_secs(batch_timeout_secs),
        })
    }

    async fn setup_channel(&mut self) -> Result<Channel, RabbitMQError> {
        if let Some(channel) = &self.channel {
            if channel.status().connected() {
                return Ok(channel.clone());
            }
        }

        // Create the new channel
        let connection = self.connection_manager.get_connection().await?;
        let channel = connection
            .create_channel()
            .await
            .map_err(|e| RabbitMQError::ChannelError(e.to_string()))?;

        // Store and return an owned value
        self.channel = Some(channel.clone());
        Ok(channel)
    }

    pub async fn start_batch_processing<F, Fut>(&mut self, processor: F) -> Result<(), RabbitMQError>
    where
        F: Fn(Vec<PaymentMessage>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + Send,
    {
        // Ensure we have a valid channel
        let channel = self.setup_channel().await?;

        // Set up the consumer
        let consumer = channel.basic_consume(
            &self.queue,
            "batch_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        ).await.map_err(|e| RabbitMQError::ConsumeError(e.to_string()))?;

        // Set up batch collection
        let mut messages = Vec::new();
        let batch_size = self.batch_size;
        let batch_timeout = self.batch_timeout;

        // Process batches
        let mut message_stream = consumer.into_stream();

        loop {
            tokio::select! {
                Some(delivery_result) = message_stream.next() => {
                    match delivery_result {
                        Ok(delivery) => {
                            match serde_json::from_slice::<PaymentMessage>(&delivery.data) {
                                Ok(payment_message) => {
                                    messages.push(payment_message);

                                    // Process batch if we've reached batch_size
                                    if messages.len() >= batch_size {
                                        let batch = std::mem::take(&mut messages);
                                        if let Err(e) = processor(batch).await {
                                            // Handle processing error
                                            log::error!("Batch processing error: {}", e);
                                        }
                                    }

                                    // Acknowledge the message
                                    if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                        return Err(RabbitMQError::AckError(e.to_string()));
                                    }
                                },
                                Err(e) => {
                                    log::error!("Message deserialization error: {}", e);
                                    // Consider whether to acknowledge, reject, or nack messages that can't be deserialized
                                    if let Err(e) = delivery.reject(BasicRejectOptions::default()).await {
                                        log::error!("Failed to reject message: {}", e);
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            log::error!("Error receiving message: {}", e);
                        }
                    }
                }

                // Process any remaining messages after timeout
                _ = tokio::time::sleep(batch_timeout) => {
                    if !messages.is_empty() {
                        let batch = std::mem::take(&mut messages);
                        if let Err(e) = processor(batch).await {
                            log::error!("Batch processing error on timeout: {}", e);
                        }
                    }
                }
            }
        }
    }

    async fn process_batch<F>(
        batch: &[PaymentMessage],
        deliveries: &[Delivery],
        processor: &std::sync::Arc<F>,
    ) -> Result<(), RabbitMQError>
    where
        F: Fn(Vec<PaymentMessage>) -> Result<(), Box<dyn std::error::Error>> + Send + Sync,
    {
        info!("Processing batch of {} messages", batch.len());
        let batch_vec: Vec<PaymentMessage> = batch.iter().cloned().collect();

        match processor(batch_vec) {
            Ok(_) => {
                // Acknowledge all messages in the batch
                for delivery in deliveries {
                    if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                        error!("Failed to acknowledge message: {}", e);
                    }
                }
                info!("Successfully processed batch of {} messages", batch.len());
                Ok(())
            }
            Err(e) => {
                error!("Error processing batch: {}", e);

                // Negative acknowledge all messages in the batch
                for delivery in deliveries {
                    if let Err(e) = delivery
                        .nack(BasicNackOptions {
                            requeue: true,
                            ..BasicNackOptions::default()
                        })
                        .await
                    {
                        error!("Failed to negatively acknowledge message: {}", e);
                    }
                }
                Err(RabbitMQError::ConsumeError(format!("Batch processing error: {}", e)))
            }
        }
    }

    pub async fn close(&mut self) -> Result<(), RabbitMQError> {
        if let Some(channel) = &self.channel {
            channel
                .close(0, "Closing batch consumer")
                .await
                .map_err(|e| RabbitMQError::ChannelError(e.to_string()))?;
        }

        self.connection_manager.close().await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    env_logger::init();

    // Load configuration from environment or use defaults
    dotenv::dotenv().ok();
    let rabbitmq_uri = std::env::var("RABBITMQ_URI")
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/%2f".to_string());

    // Example of using BatchConsumer
    let mut consumer = BatchConsumer::new(
        &rabbitmq_uri,
        "payments",
        "payment_processing",
        10, // Process in batches of 10
        5,  // Or after 5 seconds
    ).await?;

    println!("Starting batch processing...");

    // Start processing batches
    consumer.start_batch_processing(|batch: Vec<PaymentMessage>| async move {
        println!("Processing batch of {} payments", batch.len());

        // Process each payment in the batch
        for payment in batch {
            println!("  - Processing payment: {} for ${}", payment.payment_id, payment.amount);
            // Simulate processing time
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }).await?;

    Ok(())
}