// At the top of advanced_patterns.rs
use super::ConnectionManager;
use futures::stream::StreamExt; // from futures

use std::future::Future;
use std::pin::Pin;
fn tokio_executor(future: Pin<Box<dyn Future<Output = ()> + Send>>) {
    tokio::spawn(future);
}

use futures::TryStreamExt;
use lapin::{
    message::Delivery, options::*, types::FieldTable, BasicProperties, Channel, Error as LapinError,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{error, info};
use uuid::Uuid;

// Instead of:
// use crate::common::connection::ConnectionManager;

#[derive(Error, Debug)]
pub enum RabbitError {
    #[error("Failed to connect to RabbitMQ: {0}")]
    ConnectionError(#[from] LapinError),

    #[error("Failed to serialize/deserialize message: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Consumer error: {0}")]
    ConsumerError(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct PaymentMessage {
    payment_id: String,
    order_id: String,
    amount: f64,
    status: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

// Add a url field to your struct definition
struct DeadLetterPublisher {
    connection_manager: ConnectionManager,
    channel: Option<Channel>, // I see it's an Option<Channel> based on the code
    exchange: String,
    dead_letter_exchange: String,
    url: String, // Add this field
}

impl DeadLetterPublisher {
    pub async fn new(
        uri: &str,
        exchange: &str,
        dead_letter_exchange: &str,
    ) -> Result<Self, RabbitError> {
        let connection_manager = ConnectionManager::new(uri).with_reconnect_policy(5, 1000);

        Ok(DeadLetterPublisher {
            connection_manager,
            channel: None,
            exchange: exchange.to_string(),
            dead_letter_exchange: dead_letter_exchange.to_string(),
            url: uri.to_string(), // Store the URL
        })
    }

    async fn setup_channel(&mut self) -> Result<Channel, RabbitError> {
        // This checks and returns a reference to the existing channel
        // Create a new channel
        let connection = self.connection_manager.get_connection().await?;
        let channel = connection
            .create_channel()
            .await
            .map_err(|e| RabbitError::ChannelError(e.to_string()))?;

        // Store it and return a clone
        self.channel = Some(channel.clone());
        Ok(channel)
    }

    pub async fn publish(&mut self, message: &PaymentMessage) -> Result<(), RabbitError> {
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
            .map_err(|e| RabbitError::ChannelError(format!("Failed to publish message: {}", e)))?;

        info!("Published payment message: {}", message.payment_id);

        Ok(())
    }

    pub async fn close(&mut self) -> Result<(), RabbitError> {
        if let Some(channel) = &self.channel {
            channel
                .close(0, "Closing publisher")
                .await
                .map_err(|e| RabbitError::ChannelError(e.to_string()))?;
        }

        self.connection_manager.close().await?;
        Ok(())
    }
}

// Batch Processing Consumer
struct BatchConsumer {
    connection_manager: ConnectionManager,
    channel: Option<Channel>,
    queue: String,
    batch_size: usize,
    batch_timeout: Duration,
}

impl BatchConsumer {
    pub async fn new(
        uri: &str,
        queue: &str,
        batch_size: usize,
        batch_timeout_ms: u64,
    ) -> Result<Self, RabbitError> {
        let connection_manager = ConnectionManager::new(uri).with_reconnect_policy(5, 1000);

        Ok(BatchConsumer {
            connection_manager,
            channel: None,
            queue: queue.to_string(),
            batch_size,
            batch_timeout: Duration::from_millis(batch_timeout_ms),
        })
    }

    async fn setup_channel(&mut self) -> Result<Channel, RabbitError> {
        if let Some(channel) = &self.channel {
            if channel.status().connected() {
                return Ok(channel.clone()); // Return a clone instead of a reference
            }
        }

        // Create the new channel
        let connection = self.connection_manager.get_connection().await?;
        let channel = connection
            .create_channel()
            .await
            .map_err(|e| RabbitError::ChannelError(e.to_string()))?;

        // Store and return an owned value
        self.channel = Some(channel.clone());
        Ok(channel)
    }

    pub async fn start_batch_processing<F>(&mut self, processor: F) -> Result<(), RabbitError>
    where
        F: Fn(Vec<PaymentMessage>) -> Result<(), Box<dyn std::error::Error>>
            + Send
            + Sync
            + 'static,
    {
        let channel = self.setup_channel().await?;

        // Set prefetch count to batch size to optimize throughput
        channel
            .basic_qos(self.batch_size as u16, BasicQosOptions::default())
            .await
            .map_err(|e| RabbitError::ChannelError(format!("Failed to set QoS: {}", e)))?;

        // Create a consumer
        let consumer = channel
            .basic_consume(
                &self.queue,
                "batch_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| RabbitError::ConsumerError(e.to_string()))?;

        info!("Started batch consumer for queue: {}", self.queue);

        // Create channel for batch processing
        let (tx, mut rx) = mpsc::channel::<(PaymentMessage, Delivery)>(self.batch_size * 2);

        // Spawn task to collect messages
        let mut consumer_stream = consumer.into_stream();

        let tx_clone = tx.clone();

        tokio::spawn(async move {
            while let Some(delivery_result) = consumer_stream.next().await {
                match delivery_result {
                    Ok(delivery) => {
                        match serde_json::from_slice::<PaymentMessage>(&delivery.data) {
                            Ok(message) => {
                                // Send message and delivery for batching
                                if tx_clone.send((message, delivery)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Failed to deserialize message: {}", e);

                                // Reject malformed message
                                if let Err(e) =
                                    delivery.reject(BasicRejectOptions { requeue: false }).await
                                {
                                    error!("Failed to reject message: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error receiving message: {}", e);
                    }
                }
            }
        });

        // Process messages in batches
        let processor = std::sync::Arc::new(processor);

        tokio::spawn(async move {
            let mut batch = Vec::with_capacity(self.batch_size);
            let mut deliveries = Vec::with_capacity(self.batch_size);

            loop {
                let timeout = tokio::time::sleep(self.batch_timeout);
                tokio::pin!(timeout);

                tokio::select! {
                    // Either we receive a message
                    result = rx.recv() => {
                        match result {
                            Some((message, delivery)) => {
                                batch.push(message);
                                deliveries.push(delivery);

                                // Process batch if it reached the size limit
                                if batch.len() >= self.batch_size {
                                    Self::process_batch(&batch, &deliveries, &processor).await;
                                    batch.clear();
                                    deliveries.clear();
                                }
                            },
                            None => break, // Channel closed
                        }
                    }
                    // Or we hit the timeout
                    _ = &mut timeout => {
                        if !batch.is_empty() {
                            Self::process_batch(&batch, &deliveries, &processor).await;
                            batch.clear();
                            deliveries.clear();
                        }
                    }
                }
            }
        });

        Ok(())
    }

    async fn process_batch<F>(
        batch: &[PaymentMessage],
        deliveries: &[Delivery],
        processor: &std::sync::Arc<F>,
    ) where
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
            }
        }
    }

    pub async fn close(&mut self) -> Result<(), RabbitError> {
        if let Some(channel) = &self.channel {
            channel
                .close(0, "Closing batch consumer")
                .await
                .map_err(|e| RabbitError::ChannelError(e.to_string()))?;
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
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/%2f".to_string());

    // Example 1: Dead Letter Exchange and TTL
    let mut publisher =
        DeadLetterPublisher::new(&rabbitmq_uri, "payments_exchange", "dead_letter_exchange")
            .await?;

    // Publish some payment messages
    for i in 0..5 {
        let status = if i % 2 == 0 {
            "processing"
        } else {
            "completed"
        };

        let payment = PaymentMessage {
            payment_id: Uuid::new_v4().to_string(),
            order_id: format!("order-{}", i),
            amount: 100.0 * (i as f64 + 1.0),
            status: status.to_string(),
            timestamp: chrono::Utc::now(),
        };

        publisher.publish(&payment).await?;
    }

    // Example 2: Batch Processing
    let mut batch_consumer = BatchConsumer::new(
        &rabbitmq_uri,
        "payments_queue",
        10,   // Process in batches of 10
        5000, // Or after 5 seconds
    )
    .await?;

    // Start batch processing
    batch_consumer
        .start_batch_processing(|batch: Vec<PaymentMessage>| {
            println!("Processing batch of {} payments", batch.len());

            // Simulate batch processing
            for payment in &batch {
                println!(
                    "  - Payment {}: ${:.2} ({})",
                    payment.payment_id, payment.amount, payment.status
                );
            }

            // Simulate some processing time
            std::thread::sleep(Duration::from_millis(100));

            println!("Batch processing completed!");
            Ok(())
        })
        .await?;

    // Keep the application running to process messages
    tokio::time::sleep(Duration::from_secs(30)).await;

    // Close resources
    publisher.close().await?;
    batch_consumer.close().await?;

    Ok(())
}
