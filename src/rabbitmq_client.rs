use crate::common::errors::Result;
use crate::models::{ErrorPayload, RRMessage, RRMessagePayload, RRMessageType};

use crate::env::Config;
use futures_lite::StreamExt;
use lapin::{
    options::*, types::FieldTable, BasicProperties, Channel, Connection, ConnectionProperties,
};

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::{error, info};

pub struct RabbitMQClient {
    connection: Arc<Mutex<Connection>>,
    config: Config,
}

impl RabbitMQClient {
    // Changed this from private to public to match how it's being used in main.rs
    pub async fn consume(&self, queue_name: &str) -> Result<()> {
        let channel = self.create_channel().await?;
        let mut consumer = channel
            .basic_consume(
                queue_name, // Fixed: was using undeclared variable 'queue'
                "consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;
        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    let message_result = serde_json::from_slice::<RRMessage>(&delivery.data);
                    match message_result {
                        Ok(message) => {
                            info!("Consumed message: {:?}", message);
                            match message.payload {
                                RRMessagePayload::OrderCreated(order_payload) => {
                                    info!("Consumed OrderCreated message: {:?}", order_payload);
                                }
                                RRMessagePayload::UserRegistered(user_payload) => {
                                    info!("Consumed UserRegistered message: {:?}", user_payload);
                                }
                                RRMessagePayload::Error(error_payload) => {
                                    error!("Consumed Error Message: {:?}", error_payload);
                                    match error_payload {
                                        ErrorPayload::CardDeclined { order_id, reason } => {
                                            error!(
                                                "CardDeclined: OrderId: {} Reason: {}",
                                                order_id, reason
                                            );
                                        }
                                        ErrorPayload::InsufficientStock {
                                            product_id,
                                            quantity_requested,
                                            quantity_available,
                                        } => {
                                            error!("InsufficientStock: ProductId: {} requested: {} available: {}", product_id, quantity_requested, quantity_available);
                                        }
                                        ErrorPayload::PaymentFailed { order_id, reason } => {
                                            error!(
                                                "PaymentFailed: OrderId: {} Reason: {}",
                                                order_id, reason
                                            );
                                        }
                                        ErrorPayload::InvalidOrder { order_id, errors } => {
                                            error!(
                                                "InvalidOrder: OrderId: {} Errors: {:?}",
                                                order_id, errors
                                            );
                                        }
                                    }
                                }
                            }
                            delivery.ack(BasicAckOptions::default()).await.expect("ack");
                        }
                        Err(e) => {
                            error!("Failed to deserialize message: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to consume message: {}", e);
                }
            }
        }
        Ok(())
    }
    pub async fn send_error(&self, error_payload: ErrorPayload, queue: &str) -> Result<()> {
        let message = RRMessage::new(RRMessageType::Error, RRMessagePayload::Error(error_payload));
        self.publish(message, queue).await?;
        Ok(())
    }

    pub async fn new(amqp_addr: &str, config: Config) -> Result<Self> {
        let connection_properties = ConnectionProperties::default();
        let connection_result = timeout(
            config.connect_timeout(),
            Connection::connect(amqp_addr, connection_properties),
        )
        .await;
        let conn = match connection_result {
            Ok(Ok(conn)) => conn,
            Ok(Err(e)) => return Err(format!("Failed to connect to RabbitMQ: {}", e).into()),
            Err(_) => return Err("Connection to RabbitMQ timed out".into()),
        };
        Ok(Self {
            connection: Arc::new(Mutex::new(conn)),
            config,
        })
    }

    pub async fn create_channel(&self) -> Result<Channel> {
        let connection = self.connection.lock().await;
        let channel = connection.create_channel().await?;
        channel
            .basic_qos(
                self.config.rabbitmq_prefetch_count,
                BasicQosOptions::default(),
            )
            .await?;
        Ok(channel)
    }

    pub async fn publish(&self, message: RRMessage, queue: &str) -> Result<()> {
        let channel = self.create_channel().await?;
        let message_bytes = serde_json::to_vec(&message)?;
        let confirm = channel
            .basic_publish(
                "",
                queue,
                BasicPublishOptions::default(),
                message_bytes.as_ref(),
                BasicProperties::default(),
            )
            .await?;
        confirm
            .await
            .map_err(|e| format!("Failed to confirm publish: {}", e))?;
        info!("Published message: {:?}", message);
        Ok(())
    }
}
