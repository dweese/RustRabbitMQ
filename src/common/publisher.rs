use lapin::{
    options::{BasicPublishOptions, ExchangeDeclareOptions},
    types::FieldTable,
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PublishError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Exchange declaration error: {0}")]
    ExchangeError(String),

    #[error("Failed to publish message: {0}")]
    PublishError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

pub struct Publisher {
    amqp_uri: String,
    exchange: String,
    connection: Option<Connection>,
    channel: Option<Channel>,
}

impl Publisher {
    pub async fn new(amqp_uri: &str, exchange: &str) -> Result<Self, PublishError> {
        Ok(Self {
            amqp_uri: amqp_uri.to_string(),
            exchange: exchange.to_string(),
            connection: None,
            channel: None,
        })
    }

    pub async fn publish<T: Serialize>(&mut self, routing_key: &str, data: &T) -> Result<(), PublishError> {
        let payload = serde_json::to_vec(data).map_err(PublishError::SerializationError)?;
        self.publish_raw(payload, routing_key).await
    }

    async fn publish_raw(&mut self, payload: Vec<u8>, routing_key: &str) -> Result<(), PublishError> {
        let exchange = self.exchange.clone();

        // Ensure we have a working connection
        self.ensure_connected().await?;

        // Safe to unwrap as ensure_connected guarantees a valid channel
        let channel = self.channel.as_ref().unwrap();

        channel
            .basic_publish(
                &exchange,
                routing_key,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default(),
            )
            .await
            .map_err(|e| PublishError::PublishError(format!("Failed to publish message: {}", e)))?;

        Ok(())
    }

    // This method doesn't return a reference, just ensures we have a working connection
    async fn ensure_connected(&mut self) -> Result<(), PublishError> {
        // Check if we already have a working channel
        if let Some(channel) = &self.channel {
            if channel.status().connected() {
                return Ok(());
            }
        }

        // We need a new connection + channel
        let connection = Connection::connect(
            &self.amqp_uri,
            ConnectionProperties::default(),
        )
            .await
            .map_err(|e| PublishError::ConnectionError(format!("Failed to connect: {}", e)))?;

        let channel = connection
            .create_channel()
            .await
            .map_err(|e| PublishError::ChannelError(format!("Failed to create channel: {}", e)))?;

        // Declare exchange if needed
        channel
            .exchange_declare(
                &self.exchange,
                ExchangeKind::Topic,
                ExchangeDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                PublishError::ExchangeError(format!("Failed to declare exchange: {}", e))
            })?;

        // Store both connection and channel
        self.connection = Some(connection);
        self.channel = Some(channel);

        Ok(())
    }
}