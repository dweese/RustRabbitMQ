use anyhow::Result;
use lapin::{
    options::{BasicQosOptions, ConfirmSelectOptions},
    Channel, ConnectionProperties,
};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::{debug, error, info, warn};
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
        let mut guard = self.channel.lock().map_err(|_| ChannelError::LockError)?;

        // Check if we need to create a new channel
        let needs_new_channel = match &*guard {
            Some(channel) => !channel.status().connected(),
            None => true,
        };

        if needs_new_channel {
            debug!(channel_id = %self.config.id, "Creating new channel");
            self.create_channel(&mut guard).await?;
        }

        guard.as_ref()
            .map(|ch| ch.clone())
            .ok_or(ChannelError::ChannelNotAvailable)
    }

    async fn create_channel(&self, guard: &mut std::sync::MutexGuard<'_, Option<Channel>>) -> Result<(), ChannelError> {
        let channel = self.connection.create_channel().await?;

        // Configure QoS if specified
        if self.config.prefetch_count > 0 {
            debug!("Setting channel QoS to {}", self.config.prefetch_count);
            channel.basic_qos(self.config.prefetch_count, BasicQosOptions::default())
                .await
                .map_err(|e| ChannelError::ConfigurationFailed(format!("Failed to set QoS: {}", e)))?;
        }

        // Enable confirm mode if specified
        if self.config.confirm_mode {
            debug!("Enabling confirm mode for channel {}", self.config.id);
            channel.confirm_select(ConfirmSelectOptions::default())
                .await
                .map_err(|e| ChannelError::ConfigurationFailed(format!("Failed to enable confirm mode: {}", e)))?;
        }

        info!("Channel {} created and configured successfully", self.config.id);
        **guard = Some(channel);  // This should be *guard since we have a &mut MutexGuard
        Ok(())
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