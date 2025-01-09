Alright, let's build that rabbitmq_client!

Create src/rabbitmq_client.rs:

In your project's src directory, create a new file named rabbitmq_client.rs.
Add the code:

Open rabbitmq_client.rs in your editor and add the following code:

Rust

use lapin::{
    options::*, 
    types::FieldTable, 
    Connection, 
    ConnectionProperties, 
    Result
};
use futures_lite::stream::StreamExt;
use std::env;

// Import the `Message` struct from the `message` module
use crate::message::Message;

pub struct RabbitMQClient {
    conn: Connection, 
    channel: lapin::Channel,
}

impl RabbitMQClient {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Get the RabbitMQ connection string from an environment variable
        let addr = env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

        // Connect to RabbitMQ
        let conn = Connection::connect(&addr, ConnectionProperties::default()).await?;
        let channel = conn.create_channel().await?; 

        Ok(Self { conn, channel })
    }

    pub async fn publish(&self, message: Message, queue_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Declare the queue (if it doesn't exist)
        self.channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;

        // Serialize the message using serde_json
        let payload = serde_json::to_string(&message)?; 

        // Publish the message
        self.channel
            .basic_publish(
                "",
                queue_name,
                BasicPublishOptions::default(),
                payload.as_bytes().to_vec(),
                BasicProperties::default(),
            )
            .await?
            .await?; // Wait for confirmation

        println!(" [x] Sent {:?}", message);
        Ok(())
    }

    pub async fn consume(&self, queue_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Declare the queue (if it doesn't exist)
        self.channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;

        // Consume messages
        let mut consumer = self.channel
            .basic_consume(
                queue_name,
                "my_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        println!(" [*] Waiting for messages.");
        while let Some(delivery) = consumer.next().await {
            let delivery = delivery.expect("error caught in consumer");
            let message = serde_json::from_slice::<Message>(&delivery.data)?;
            println!(" [x] Received {:?}", message);
            delivery.ack(BasicAckOptions::default()).await?;
        }

        Ok(())
    }
}
