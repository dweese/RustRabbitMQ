// Another advanced example for advanced_patterns.rs

use anyhow::Result;
use futures::StreamExt;
use lapin::{
    message::Delivery,
    options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions},
    Channel
};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::VecDeque,
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};

use thiserror::Error;
use tokio::{
    sync::{mpsc, Semaphore},
    time,
};
use tracing::{ error, info, instrument };
use uuid::Uuid;

use crate::common::ConnectionManager;

// 1. Zero-copy deserialization with lifetimes
// This allows us to process messages without allocating a new string for each field
#[derive(Debug, Deserialize)]
struct BorrowedMessage<'a> {
    #[serde(borrow)]
    id: Cow<'a, str>,
    #[serde(borrow)]
    payload: Cow<'a, str>,
    priority: u8,
    timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct OwnedMessage {
    id: String,
    payload: String,
    priority: u8,
    timestamp: i64,
}

impl<'a> From<BorrowedMessage<'a>> for OwnedMessage {
    fn from(borrowed: BorrowedMessage<'a>) -> Self {
        Self {
            id: borrowed.id.into_owned(),
            payload: borrowed.payload.into_owned(),
            priority: borrowed.priority,
            timestamp: borrowed.timestamp,
        }
    }
}

// 2. Custom error type for advanced error handling
#[derive(Error, Debug)]
enum MessageProcessingError {
    #[error("Failed to deserialize message: {0}")]
    DeserializationError(#[from] serde_json::Error),

    #[error("Failed to process message: {0}")]
    ProcessingError(String),

    #[error("Message queue is full")]
    QueueFull,

    #[error("Channel error: {0}")]
    ChannelError(#[from] lapin::Error),

    #[error("Timeout after {0:?}")]
    Timeout(Duration),
}

// 3. Rate-limited stream processor with backpressure
struct RateLimitedProcessor {
    connection_manager: ConnectionManager,
    channel: Option<Channel>,
    queue_name: String,
    semaphore: Arc<Semaphore>,
    batch_size: usize,
    batch_timeout: Duration,
    processing_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
}

impl RateLimitedProcessor {
    pub async fn new(
        uri: &str,
        queue_name: &str,
        concurrent_limit: usize,
        batch_size: usize,
        batch_timeout_ms: u64,
    ) -> Result<Self> {
        Ok(Self {
            connection_manager: ConnectionManager::new(uri),
            channel: None,
            queue_name: queue_name.to_string(),
            semaphore: Arc::new(Semaphore::new(concurrent_limit)),
            batch_size,
            batch_timeout: Duration::from_millis(batch_timeout_ms),
            processing_queue: Arc::new(Mutex::new(VecDeque::new())),
        })
    }

    async fn setup(&mut self) -> Result<&Channel> {
        if self.channel.is_none() {
            let channel = self
                .connection_manager
                .get_connection()
                .await?
                .create_channel()
                .await?;
            self.channel = Some(channel);
        }
        Ok(self.channel.as_ref().unwrap())
    }

    // Process messages with backpressure and rate limiting
    #[instrument(skip(self, processor))]
    pub async fn process_messages<F, Fut>(&mut self, processor: F) -> Result<()>
    where
        F: Fn(OwnedMessage) -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        // Clone semaphore BEFORE calling setup()
        let semaphore = Arc::clone(&self.semaphore);
        let batch_size = self.batch_size;
        let queue_name = self.queue_name.clone();

        // Now call setup
        let channel = self.setup().await?;

        // Set up a consumer with prefetch limit for backpressure
        channel
            .basic_qos(batch_size as u16, Default::default()) // Use local variable here
            .await?;

        let mut consumer = channel
            .basic_consume(
                &queue_name,
                &format!("processor-{}", Uuid::new_v4()),
                BasicConsumeOptions::default(),
                Default::default(),
            )
            .await?;
        // Create a channel to send batches of messages for processing
        let (batch_tx, mut batch_rx) =
            mpsc::channel::<Vec<(lapin::message::Delivery, OwnedMessage)>>(100);

        // Process batches
        let processor_clone = processor.clone();
        // let channel_clone = channel.clone();

        // Spawn a task to collect and process batches
        tokio::spawn(async move {
            while let Some(batch) = batch_rx.recv().await {
                // Acquire semaphore permits for the whole batch
                let permit = match semaphore.acquire_many(batch.len() as u32).await {
                    Ok(permit) => permit,
                    Err(e) => {
                        error!("Failed to acquire semaphore: {}", e);
                        continue;
                    }
                };

                for (delivery, message) in batch {
                    let processor = processor_clone.clone();

                    // Process the message (now accepting OwnedMessage)
                    match processor(message).await {
                        Ok(_) => {
                            // Acknowledge message
                            if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                error!("Failed to acknowledge message: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Error processing message: {}", e);
                            // Negative acknowledgment
                            if let Err(e) = delivery.nack(BasicNackOptions::default()).await {
                                error!("Failed to negative-acknowledge message: {}", e);
                            }
                        }
                    }
                }

                // Release the semaphore permits
                drop(permit);
            }
        });

        // Collect messages into batches
        // Change the type declaration of current_batch
        let mut current_batch: Vec<(Delivery, OwnedMessage)> = Vec::with_capacity(self.batch_size);

        let mut batch_timer = time::interval(self.batch_timeout);

        info!(
            "Starting to process messages from queue {}",
            self.queue_name
        );

        loop {
            tokio::select! {
                            Some(delivery_result) = consumer.next() => {
                                match delivery_result {
                                    Ok(delivery) => {
                                        // Try to parse as a borrowed message for zero-copy
            match serde_json::from_slice::<OwnedMessage>(&delivery.data) {
                Ok(message) => {
                    current_batch.push((delivery, message)); // Now we can move delivery safely


                                                // If batch is full, send it for processing
                                                if current_batch.len() >= self.batch_size {
                                                    if let Err(_) = batch_tx.send(std::mem::replace(&mut current_batch, Vec::with_capacity(self.batch_size))).await {
                                                        error!("Failed to send batch for processing, receiver dropped");
                                                        break;
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error!("Failed to deserialize message: {}", e);
                                                if let Err(e) = delivery.nack(BasicNackOptions::default()).await {
                                                    error!("Failed to negative-acknowledge message: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("Error receiving message: {}", e);
                                    }
                                }
                            }
                            _ = batch_timer.tick() => {
                                // Send any pending messages in the batch
                                if !current_batch.is_empty() {
                                    if let Err(_) = batch_tx.send(std::mem::replace(&mut current_batch, Vec::with_capacity(self.batch_size))).await {
                                        error!("Failed to send batch for processing, receiver dropped");
                                        break;
                                    }
                                }
                            }
                            else => {
                                error!("Consumer or timer stream ended unexpectedly");
                                break;
                            }
                        }
        }

        Ok(())
    }
}

// 4. Usage example with custom processing logic
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create the processor
    let mut processor = RateLimitedProcessor::new(
        "amqp://guest:guest@localhost:5672",
        "high_volume_queue",
        100, // Max concurrent messages
        20,  // Batch size
        500, // Batch timeout in ms
    )
    .await?;

    // Define a processor function that uses zero-copy deserialization
    let process = |message: OwnedMessage| async move {
        info!(
            "Processing message {} with priority {}",
            message.id, message.priority
        );

        // Example processing logic - only allocate if needed
        if message.priority > 5 {
            // For high priority messages, we might need to convert to owned
            let owned_message: OwnedMessage = message.into();
            // ... do something with owned_message that requires ownership
        } else {
            // For low priority messages, work with borrowed data
            if message.payload.contains("error") {
                return Err(anyhow::anyhow!("Error found in message payload"));
            }
        }

        // Simulate some async processing work
        tokio::time::sleep(Duration::from_millis(50)).await;

        Ok(())
    };

    // Start processing messages
    processor.process_messages(process).await?;

    Ok(())
}
