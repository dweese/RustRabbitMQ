use futures_lite::stream::StreamExt;
use lapin::{
    options::*, types::FieldTable, BasicProperties, Channel, Connection, ConnectionProperties,
    Error, Result,
};
use std::env;
use tracing::{error, info};

use crate::message::Message;

#[derive(Clone)] // Add Clone here
pub struct RabbitMQClient {
    channel: Channel,
}

impl RabbitMQClient {
    pub async fn new() -> Result<Self> {
        let addr = env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

        info!("Connecting to RabbitMQ at {}", addr);
        let conn = Connection::connect(&addr, ConnectionProperties::default())
            .await
            .map_err(|e| {
                error!("Failed to connect to RabbitMQ: {}", e);
                e
            })?;

        info!("Creating channel");
        let channel = conn.create_channel().await.map_err(|e| {
            error!("Failed to create channel: {}", e);
            e
        })?;

        Ok(Self { channel })
    }

    pub async fn publish(&self, message: Message, queue_name: &str) -> Result<()> {
        info!("Publishing message to queue {}", queue_name);

        // Declare the queue
        self.channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                error!("Failed to declare queue {}: {}", queue_name, e);
                e
            })?;

        let payload = serde_json::to_string(&message).map_err(|e| {
            error!("Failed to serialize message: {}", e);
            Error::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        self.channel
            .basic_publish(
                "",
                queue_name,
                BasicPublishOptions::default(),
                payload.as_bytes(), // Remove .to_vec()
                BasicProperties::default(),
            )
            .await
            .map_err(|e| {
                error!("Failed to publish message: {}", e);
                e
            })?
            .await
            .map_err(|e| {
                error!("Failed to confirm publish: {}", e);
                e
            })?;

        info!("Message published successfully");
        Ok(())
    }

    pub async fn consume(&self, queue_name: &str) -> Result<()> {
        info!("Consuming messages from queue {}", queue_name);

        // Declare the queue
        self.channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                error!("Failed to declare queue {}: {}", queue_name, e);
                e
            })?;

        let mut consumer = self
            .channel
            .basic_consume(
                queue_name,
                "my_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                error!("Failed to start consuming: {}", e);
                e
            })?;

        info!("Waiting for messages");
        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    let message =
                        serde_json::from_slice::<Message>(&delivery.data).map_err(|e| {
                            error!("Failed to deserialize message: {}", e);
                            Error::from(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                e.to_string(),
                            ))
                        })?;
                    info!("Received message: {:?}", message);
                    delivery
                        .ack(BasicAckOptions::default())
                        .await
                        .map_err(|e| {
                            error!("Failed to acknowledge message: {}", e);
                            e
                        })?;
                }
                Err(e) => {
                    error!("Error receiving message: {}", e);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::rabbitmq_client::RabbitMQClient;

    #[tokio::test]
    async fn test_rabbitmq_client_new() {
        // This might require setting up a mock RabbitMQ server or using a real one
        // For simplicity, we'll just check if it compiles and doesn't panic
        let _client = RabbitMQClient::new().await.unwrap();
    }
}
