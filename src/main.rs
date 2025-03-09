mod rabbitmq_client;
mod message;
mod env;
mod rabbitmq;

use crate::env::Config;  // If env.rs is directly in the src directory
use crate::{message::RRMessage, rabbitmq_client::RabbitMQClient};
use std::error::Error;
use tracing::info;
use std::sync::Arc; // Import Arc


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let amqp_addr = "amqp://user_rust:HwhYg1Lw8wUh@localhost:5672/%2fvhost_rust";

    let rabbitmq_client = Arc::new(RabbitMQClient::new(amqp_addr).await?);


    let producer_client = rabbitmq_client.clone();
    tokio::spawn(async move {
        if let Err(e) = producer_task(producer_client).await {
            eprintln!("Producer task error: {}", e);
        }
    });

    let consumer_client = rabbitmq_client.clone();
    tokio::spawn(async move {
        if let Err(e) = consumer_task(consumer_client).await {
            eprintln!("Consumer task error: {}", e);
        }
    });

    futures::future::pending::<()>().await;

    Ok(())
}

async fn producer_task(rabbitmq_client: Arc<RabbitMQClient>) -> Result<(), Box<dyn Error>> { // Corrected signature
    let message = RRMessage {
        content: "Message from producer".to_string(),
        message_type: Some("producer_message".to_string()),
    };
    rabbitmq_client.publish(message, "my_queue").await?; // Access methods through the Arc
    info!("Producer published message");
    Ok(())
}

async fn consumer_task(rabbitmq_client: Arc<RabbitMQClient>) -> Result<(), Box<dyn Error>> {  // Corrected signature
    info!("Consumer started");
    // ... (Logic to consume messages using rabbitmq_client.consume) ...   // Access methods through the Arc

    Ok(())
}






// tests be;pw
mod tests {
    use lapin::{Connection, ConnectionProperties};
    use super::*; // imports from the current file

    #[test]
    fn test_connection_construction() {
        let amqp_addr = "amqp://user_rust:HwhYg1Lw8wUh@localhost:5672/vhost_rust";

        // Mock or stub the actual connection for isolated unit testing
        let connection_properties = ConnectionProperties::default();



        let connection_result = Connection::connect(amqp_addr, ConnectionProperties::default());


        // Assert that the connection object is constructed as expected.
        assert!(connection_result.is_ok()); // Simple verification.

        Ok(())

    }
}
