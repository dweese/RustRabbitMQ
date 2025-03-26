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
    message::Delivery, options::*, types::FieldTable, BasicProperties, Channel,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use tracing::{error, info};
use uuid::Uuid;
use crate::common::errors::RabbitError;


// Instead of:
// use crate::common::connection::ConnectionManager;

#[derive(Debug, Serialize, Deserialize, Clone)]
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

    pub async fn start_batch_processing<F, Fut>(&mut self, processor: F) -> Result<(), RabbitError>
    where
        F: Fn(Vec<PaymentMessage>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), Box<dyn std::error::Error>>> + Send,
    {
        // Ensure we have a valid channel
        if self.channel.is_none() {
            self.channel = Some(self.setup_channel().await?);
        }

        let channel = self.channel.as_ref().unwrap();

        // Set up the consumer
        let consumer = channel.basic_consume(
            &self.queue,
            "batch_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        ).await.map_err(|e| RabbitError::ConsumeError(e.to_string()))?;

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
                                    return Err(RabbitError::AckError(e.to_string()));
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
    // }

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
}
