mod message;
mod rabbitmq_client;

use crate::{message::Message, rabbitmq_client::RabbitMQClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the RabbitMQ client
    let client = RabbitMQClient::new().await?;

    // Create a message
    let message = Message {
        content: "Hello, world!".to_string(),
        message_type: Some("greeting".to_string()),
    };

    // Publish the message
    client.publish(message.clone(), "hello").await?;

    // Consume messages from the same queue
    client.consume("hello").await?;

    Ok(())
}