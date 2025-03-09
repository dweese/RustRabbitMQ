use crate::RRMessage;
use futures_lite::StreamExt;
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
// Import Arc
use tokio::sync::Mutex;  // Use Mutex for channel access


#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub content: String,
    pub message_type: Option<String>,
}


//RabbitMQ client
pub struct RabbitMQClient {
    connection: Arc<Mutex<Connection>>,
}



impl RabbitMQClient {
    pub async fn new(amqp_addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let conn = Connection::connect(
            amqp_addr,
            ConnectionProperties::default(),
        )
            .await?;


        Ok(Self { connection: Arc::new(Mutex::new(conn)) })
    }



    pub async fn create_channel(&self) -> Result<Channel, Box<dyn std::error::Error>> {
        let mut connection = self.connection.lock().await; // Lock connection
        let channel = connection.create_channel().await?;  // Create new channel each time

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



#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    tracing_subscriber::fmt::init();
    // Set the AMQP address using an environment variable for flexibility
    // In a real application, you'd manage sensitive data (like password) securely.
    // Consider using configuration files or other appropriate methods.






    std::env::set_var("AMQP_ADDR", "amqp://user:password@host:port/%2f");
    let amqp_addr = "amqp://user_rust:HwhYg1Lw8wUh@localhost:5672/vhost_rust";
    let rabbitmq_client = Arc::new(RabbitMQClient::new(amqp_addr).await?);

    let consumer_client = rabbitmq_client.clone(); //  consumer client
    tokio::spawn(async move {
        if let Err(e) = consumer_client.consume("test_queue").await {
            eprintln!("Consumer error: {}", e);
        }
    });



    let producer_client = rabbitmq_client.clone();
    let producer_client = rabbitmq_client.clone();
    tokio::spawn(async move {
        let message = RRMessage {
            content: "Example Message".to_string(),
            message_type: Some("test".to_string()),
        };
        if let Err(e) = producer_client.publish(message, "test_queue").await {
            eprintln!("Publish error: {}", e);
        }
    });




    // Keep the main thread alive (essential for the tasks to run)
    tokio::signal::ctrl_c().await?;
    println!("Exiting");
    Ok(())

}
