mod message;
mod rabbitmq_client;

use crate::{message::Message, rabbitmq_client::RabbitMQClient};
use tracing::info;
use tracing_subscriber::fmt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

   // Set up tracing subscriber
    fmt::init();

    // Initialize the RabbitMQ client
    let client = RabbitMQClient::new().await?;
    
    info!("RabbitMQ client initialized"); // Tracing call

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