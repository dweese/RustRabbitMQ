use lapin::{
    options::*, types::FieldTable, BasicProperties, Channel, Error as LapinError,
    ExchangeKind,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{error, info};
use uuid::Uuid;

// With this (depending on what you need):
use super::connection::ConnectionManager; // Access the ConnectionManager from connection.rs module

#[derive(Error, Debug)]
pub enum PublishError {
    #[error("Failed to connect to RabbitMQ: {0}")]
    ConnectionError(#[from] LapinError),

    #[error("Failed to serialize message: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Failed to publish message: {0}")]
    PublishError(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct OrderMessage {
    order_id: String,
    customer_id: String,
    items: Vec<String>,
    total: f64,
    timestamp: chrono::DateTime<chrono::Utc>,
}

struct Publisher {
    connection_manager: ConnectionManager,
    channel: Option<Channel>,
    exchange: String,
}

impl Publisher {
    pub async fn new(uri: &str, exchange: &str) -> Result<Self, PublishError> {
        let connection_manager = ConnectionManager::new(uri).with_reconnect_policy(5, 1000);

        Ok(Publisher {
            connection_manager,
            channel: None,
            exchange: exchange.to_string(),
        })
    }

    async fn get_channel(&mut self) -> Result<&Channel, PublishError> {
        if let Some(channel) = &self.channel {
            if channel.status().connected() {
                return Ok(channel);
            }
        }

        let connection = self.connection_manager.get_connection().await?;
        let channel = connection
            .create_channel()
            .await
            .map_err(|e| PublishError::ChannelError(e.to_string()))?;

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
                PublishError::ChannelError(format!("Failed to declare exchange: {}", e))
            })?;

        self.channel = Some(channel);
        Ok(self.channel.as_ref().unwrap())
    }

    pub async fn publish<T: Serialize>(
        &mut self,
        routing_key: &str,
        message: &T,
    ) -> Result<(), PublishError> {
        let payload = serde_json::to_vec(message)?;
        let channel = self.get_channel().await?;

        let properties = BasicProperties::default()
            .with_message_id(Uuid::new_v4().to_string().into())
            .with_content_type("application/json".into())
            .with_timestamp(chrono::Utc::now().timestamp() as u64);

        channel
            .basic_publish(
                &self.exchange,
                routing_key,
                BasicPublishOptions::default(),
                &payload,
                properties,
            )
            .await
            .map_err(|e| PublishError::PublishError(e.to_string()))?;

        info!(
            "Published message to exchange '{}' with routing key '{}'",
            self.exchange, routing_key
        );

        Ok(())
    }

    pub async fn close(&mut self) -> Result<(), PublishError> {
        if let Some(channel) = &self.channel {
            channel
                .close(0, "Closing publisher")
                .await
                .map_err(|e| PublishError::ChannelError(e.to_string()))?;
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

    // Create publisher
    let mut publisher = Publisher::new(&rabbitmq_uri, "orders").await?;

    // Create and publish a sample order
    let order = OrderMessage {
        order_id: Uuid::new_v4().to_string(),
        customer_id: "customer-123".to_string(),
        items: vec!["product-1".to_string(), "product-2".to_string()],
        total: 59.99,
        timestamp: chrono::Utc::now(),
    };

    // Publish the message
    publisher.publish("new_order", &order).await?;
    info!("Successfully published order: {:?}", order);

    // Gracefully close the publisher
    publisher.close().await?;

    Ok(())
}
