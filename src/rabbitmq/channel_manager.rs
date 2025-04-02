use anyhow::Result;
use lapin::{
    Channel, ConnectionProperties,
};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::{debug, error, info, };
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum ChannelError {
    #[error("Failed to connect to RabbitMQ: {0}")]
    ConnectionFailed(#[from] lapin::Error),

    #[error("Channel is not available")]
    ChannelNotAvailable,

    #[error("Failed to configure channel: {0}")]
    ConfigurationFailed(String),

    #[error("Channel lock acquisition failed")]
    LockError,
}

/// Configuration options for a RabbitMQ channel
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    /// Number of unacknowledged messages allowed (0 means unlimited)
    pub prefetch_count: u16,

    /// Whether to use publisher confirms
    pub confirm_mode: bool,

    /// Channel identifier for logging (defaults to a UUID)
    pub id: String,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            prefetch_count: 10,
            confirm_mode: true,
            id: format!("channel-{}", Uuid::new_v4().to_string()[..8].to_string()),
        }
    }
}

/// Manages a RabbitMQ channel and its configuration
pub struct ChannelManager {
    connection: Arc<lapin::Connection>,
    channel: Mutex<Option<Channel>>,
    config: ChannelConfig,
}

impl ChannelManager {
    /// Create a new ChannelManager with an existing connection
    pub fn new(connection: Arc<lapin::Connection>, config: ChannelConfig) -> Self {
        debug!(
            channel_id = %config.id,
            prefetch = %config.prefetch_count,
            confirm = %config.confirm_mode,
            "Creating channel manager"
        );

        Self {
            connection,
            channel: Mutex::new(None),
            config,
        }
    }

    /// Create a new ChannelManager by establishing a connection
    pub async fn connect(uri: &str, config: ChannelConfig) -> Result<Self, ChannelError> {
        info!(channel_id = %config.id, "Connecting to RabbitMQ at {}", uri);
        let connection = lapin::Connection::connect(
            uri,
            ConnectionProperties::default(),
        ).await?;

        debug!("Successfully connected to RabbitMQ");
        Ok(Self::new(Arc::new(connection), config))
    }

    /// Get a channel, creating one if needed
    /// Returns a cloned Channel that can be used independently
    pub async fn get_channel(&self) -> Result<Channel, ChannelError> {
        let guard = self.channel.lock().map_err(|_| ChannelError::LockError)?;

        // Check if we need to create a new channel
        let needs_new_channel = match &*guard {
            Some(channel) => !channel.status().connected(),
            None => true,
        };

        if needs_new_channel {
            debug!(channel_id = %self.config.id, "Creating new channel");
            self.connection.create_channel().await?;

        }

        guard.as_ref()
            .map(|ch| ch.clone())
            .ok_or(ChannelError::ChannelNotAvailable)
    }

    /// Check if the channel is in a healthy state
    pub fn is_healthy(&self) -> Result<bool, ChannelError> {
        let guard = self.channel.lock().map_err(|_| ChannelError::LockError)?;

        Ok(match &*guard {
            Some(channel) => channel.status().connected(),
            None => false,
        })
    }

    /// Return the channel ID for logging
    pub fn id(&self) -> &str {
        &self.config.id
    }
}

// Usage example
async fn example_usage() -> Result<(), anyhow::Error> {
    let config = ChannelConfig {
        prefetch_count: 50,
        confirm_mode: true,
        id: "my-app-channel".to_string(),
    };

    let manager = ChannelManager::connect("amqp://guest:guest@localhost:5672", config).await?;

    // Get a channel for use
    let channel = manager.get_channel().await?;

    // Use the channel for RabbitMQ operations
    let _queue = channel.queue_declare(
        "hello",
        lapin::options::QueueDeclareOptions::default(),
        lapin::types::FieldTable::default(),
    ).await?;

    Ok(())
}

// Your new integrated example
pub async fn integrated_example() -> Result<(), anyhow::Error> {
    use lapin::options::{
        BasicConsumeOptions, BasicPublishOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
    };
    use lapin::types::{AMQPValue, FieldTable};
    use lapin::BasicProperties;
    use std::time::Duration;
    use tokio::time;
    use uuid::Uuid;
    use futures::StreamExt;
    use log::{info, error};

    // Setup connection with RabbitMQ
    let config = ChannelConfig {
        prefetch_count: 10,
        confirm_mode: true,
        id: "integrated-example".to_string(),
    };

    info!("Starting integrated RabbitMQ example");
    let manager = ChannelManager::connect("amqp://guest:guest@localhost:5672", config).await?;

    // Check channel health
    if !manager.is_healthy()? {
        error!("Channel is not healthy after connection");
        return Err(anyhow::anyhow!("Failed to establish healthy channel"));
    }

    // Get a channel for declaring exchange and queue
    let channel = manager.get_channel().await?;
    let exchange_name = "example.exchange";
    let queue_name = "example.queue";
    let routing_key = "example.key";

    // Declare exchange
    channel.exchange_declare(
        exchange_name,
        lapin::ExchangeKind::Topic,
        ExchangeDeclareOptions {
            durable: true,
            ..ExchangeDeclareOptions::default()
        },
        FieldTable::default(),
    ).await?;

    // Declare queue
    let queue = channel.queue_declare(
        queue_name,
        QueueDeclareOptions {
            durable: true,
            ..QueueDeclareOptions::default()
        },
        FieldTable::default(),
    ).await?;

    info!("Declared queue {} with {} messages", queue_name, queue.message_count());

    // Bind queue to exchange
    channel.queue_bind(
        queue_name,
        exchange_name,
        routing_key,
        QueueBindOptions::default(),
        FieldTable::default(),
    ).await?;

    // Setup consumer
    let consumer_tag = format!("consumer-{}", Uuid::new_v4().to_string()[..8].to_string());
    let mut consumer = channel.basic_consume(
        queue_name,
        &consumer_tag,
        BasicConsumeOptions::default(),
        FieldTable::default(),
    ).await?;

    // Start consumer task
    let consumer_handle = tokio::spawn(async move {
        info!("Consumer started. Waiting for messages...");
        while let Some(delivery) = consumer.next().await {
            if let Ok(delivery) = delivery {
                let data = String::from_utf8_lossy(&delivery.data);
                info!("Received message: {}", data);

                // Acknowledge the message
                delivery.ack(lapin::options::BasicAckOptions::default()).await
                    .map_err(|e| error!("Failed to acknowledge message: {}", e))
                    .ok();
            }
        }
    });

    // Get a new channel for publishing (to demonstrate channel reuse)
    let publish_channel = manager.get_channel().await?;

    // Publish a few messages
    for i in 1..=5 {
        let payload = format!("Hello from integrated example: message {}", i);

        let mut headers = FieldTable::default();
        headers.insert("message_id".into(), AMQPValue::LongString(format!("msg-{}", i).into()));

        publish_channel.basic_publish(
            exchange_name,
            routing_key,
            BasicPublishOptions::default(),
            payload.as_bytes(),
            BasicProperties::default()
                .with_content_type("text/plain".into())
                .with_headers(headers),
        ).await?;



        info!("Published message {}", i);
        time::sleep(Duration::from_millis(500)).await;
    }

    // Wait a bit to allow messages to be processed
    time::sleep(Duration::from_secs(2)).await;

    // Cleanly terminate the example
    info!("Example complete");
    consumer_handle.abort(); // Stop the consumer task

    Ok(())
}