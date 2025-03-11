use crate::message::RRMessage;
use futures_lite::StreamExt;
use lapin::{
    options::*,
    types::FieldTable,
    BasicProperties,
    Channel,
    Connection,
    ConnectionProperties,
};
use tokio::time::timeout;
use tracing::{error, info}; // For structured logging
use std::sync::Arc;
use serde::{Deserialize, Serialize};
// Import Arc
use tokio::sync::Mutex;  // Use Mutex for channel access

use crate::env::Config;


#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub content: String,
    pub message_type: Option<String>,
}


//RabbitMQ client
pub struct RabbitMQClient {
    connection: Arc<Mutex<Connection>>,
    config : Config
}



impl RabbitMQClient {
    pub async fn new(amqp_addr: &str, config : Config) -> Result<Self, Box<dyn std::error::Error>> {

        let connection_properties = ConnectionProperties::default()
            .with_connect_timeout(config.connect_timeout())
            .with_heartbeat(config.heartbeat());
        let conn = Connection::connect(
            amqp_addr,
            connection_properties,
        )
            .await?;


        Ok(Self { connection: Arc::new(Mutex::new(conn)), config:config })
    }



    pub async fn create_channel(&self) -> Result<Channel, Box<dyn std::error::Error>> {
        let connection = self.connection.lock().await; // Lock connection
        let channel = connection.create_channel().await?;  // Create new channel each time
        channel
            .basic_qos(self.config.rabbitmq_prefetch_count, BasicQosOptions::default())
            .await?;
        Ok(channel)
    }

    pub async fn publish(&self, message: RRMessage, queue: &str) -> Result<(), Box<dyn std::error::Error>> {
        let channel = self.create_channel().await?;


        let message_bytes = serde_json::to_vec(&message)?; // Use serde_json

        let confirm = channel.basic_publish(
            "",
            queue,
            BasicPublishOptions::default(),
            message_bytes.as_ref(), // Use as_ref() to avoid cloning
            BasicProperties::default(),
        ).await?;


        confirm.await.map_err(|e| format!("Failed to confirm publish: {}",e))?;  //unwrap to check for errors
        info!("Published message: {:?}", message); // Log successful publish

        Ok(())

    }



    pub async fn consume(&self, queue: &str) -> Result<(), Box<dyn std::error::Error>> {
        let channel = self.create_channel().await?;


        let mut consumer = channel.basic_consume(
            queue,
            "consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        ).await?;


        while let Some(delivery) = consumer.next().await {

            match delivery {
                Ok(delivery) => {
                    let message_result = serde_json::from_slice::<Message>(&delivery.data);
                    match message_result {
                        Ok(message) => {
                            info!("Consumed message: {:?}", message);

                            delivery.ack(BasicAckOptions::default()).await.expect("ack");
                        },
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