// src/common/amqp_client.rs

use std::error::Error;
use std::fmt::Debug;
use async_trait::async_trait;
use lapin::{Connection, ConnectionProperties, Channel};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AmqpError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Channel error: {0}")]
    ChannelError(String),
}

pub struct AmqpClient {
    amqp_uri: String,
    connection: Option<Connection>,
    channel: Option<Channel>,
}

/// Trait for AMQP client operations
#[async_trait]
pub trait AmqpClientTrait {
    /// Error type associated with this client
    type Error: Error + Debug;

    /// Create a new client with the given URI
    fn new(amqp_uri: &str) -> Self where Self: Sized;

    /// Ensure the client is connected
    async fn ensure_connected(&mut self) -> Result<(), Self::Error>;

    /// Perform operations with the channel
    async fn do_something_with_channel(&mut self) -> Result<(), Self::Error>;
}

impl AmqpClient {
    pub fn new(amqp_uri: &str) -> Self {
        Self {
            amqp_uri: amqp_uri.to_string(),
            connection: None,
            channel: None,
        }
    }

    // This method ensures we have a working connection
    async fn ensure_connected(&mut self) -> Result<(), AmqpError> {
        // Check if we already have a working channel
        if let Some(channel) = &self.channel {
            if channel.status().connected() {
                return Ok(());
            }
        }

        // Setup new connection and channel
        let connection = Connection::connect(
            &self.amqp_uri,
            ConnectionProperties::default(),
        )
            .await
            .map_err(|e| AmqpError::ConnectionError(format!("Failed to connect: {}", e)))?;

        let channel = connection
            .create_channel()
            .await
            .map_err(|e| AmqpError::ChannelError(format!("Failed to create channel: {}", e)))?;

        // Store both connection and channel
        self.connection = Some(connection);
        self.channel = Some(channel);

        Ok(())
    }

    // Use this pattern for any method that needs access to the channel
    pub async fn do_something_with_channel(&mut self) -> Result<(), AmqpError> {
        // First ensure we have a valid connection & channel
        self.ensure_connected().await?;

        // // Now we can safely use the channel
        // let channel = self.channel.as_ref().unwrap();

        // Do something with channel...

        Ok(())
    }
}

// Implement the trait for AmqpClient
#[async_trait]
impl AmqpClientTrait for AmqpClient {
    type Error = AmqpError;

    fn new(amqp_uri: &str) -> Self {
        Self::new(amqp_uri)
    }

    async fn ensure_connected(&mut self) -> Result<(), Self::Error> {
        self.ensure_connected().await
    }

    async fn do_something_with_channel(&mut self) -> Result<(), Self::Error> {
        self.do_something_with_channel().await
    }
}