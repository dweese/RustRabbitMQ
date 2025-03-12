use crate::message::{RRMessage, RRMessagePayload, ErrorPayload, RRMessageType}; // Added all the correct types.
use futures_lite::StreamExt;
use crate::env::Config;
use lapin::{
    options::*,
    types::FieldTable,
    BasicProperties,
    Channel,
    Connection,
    ConnectionProperties,
};
use tracing::{error, info}; // For structured logging
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;  // Use Mutex for channel access
use tokio::time::{timeout};
//RabbitMQ client
pub struct RabbitMQClient {
    connection: Arc<Mutex<Connection>>,
    config: Config,
}
impl RabbitMQClient {
    pub async fn new(amqp_addr: &str, config: Config) -> Result<Self, Box<dyn std::error::Error>> {
        let connection_properties = ConnectionProperties::default(); // removed with_heartbeat
        // Wrap the connection attempt in a timeout
        let connection_result = timeout(
            config.connect_timeout(), // Use the configured timeout duration
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
    pub async fn create_channel(&self) -> Result<Channel, Box<dyn std::error::Error>> {
        let connection = self.connection.lock().await; // Lock connection
        let channel = connection.create_channel().await?; // Create new channel each time
        channel
            .basic_qos(self.config.rabbitmq_prefetch_count, BasicQosOptions::default()) // Added
            .await?;
        Ok(channel)
    }
    pub async fn publish(
        &self,
        message: RRMessage, // Correct RRMessage type
        queue: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let channel = self.create_channel().await?;
        let message_bytes = serde_json::to_vec(&message)?; // Use serde_json
        let confirm = channel
            .basic_publish(
                "",
                queue,
                BasicPublishOptions::default(),
                message_bytes.as_ref(), // Use as_ref() to avoid cloning
                BasicProperties::default(),
            )
            .await?;
        confirm
            .await
            .map_err(|e| format!("Failed to confirm publish: {}", e))?; //unwrap to check for errors
        info!("Published message: {:?}", message); // Log successful publish
        Ok(())
    }
    pub async fn send_error(
        &self,
        error_payload: ErrorPayload,
        queue: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = RRMessage::new(RRMessageType::Error, RRMessagePayload::Error(error_payload));
        self.publish(message, queue).await?;
        Ok(())
    }
    pub async fn consume(&self, queue: &str) -> Result<(), Box<dyn std::error::Error>> {
        let channel = self.create_channel().await?;
        let mut consumer = channel
            .basic_consume(
                queue,
                "consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;
        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    let message_result = serde_json::from_slice::<RRMessage>(&delivery.data); // change the type here
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
                                        ErrorPayload::CardDeclined {order_id, reason} => {
                                            error!("CardDeclined: OrderId: {} Reason: {}", order_id, reason);
                                        }
                                        ErrorPayload::InsufficientStock { product_id, quantity_requested, quantity_available } => {
                                            error!("InsufficientStock: ProductId: {} requested: {} available: {}", product_id, quantity_requested, quantity_available);
                                        }
                                        ErrorPayload::PaymentFailed { order_id, reason } => {
                                            error!("PaymentFailed: OrderId: {} Reason: {}", order_id, reason);
                                        }
                                        ErrorPayload::InvalidOrder { order_id, errors } => {
                                            error!("InvalidOrder: OrderId: {} Errors: {:?}", order_id, errors);
                                        }
                                    }
                                }
                            }
                            delivery
                                .ack(BasicAckOptions::default())
                                .await
                                .expect("ack");
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
}
