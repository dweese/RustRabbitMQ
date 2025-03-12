mod rabbitmq_client;
mod message;
mod env;
mod rabbitmq;
use uuid::Uuid;
use crate::env::Config;  // If env.rs is directly in the src directory

use crate::{message::RRMessage,
            message::RRMessageType,
            message::RRMessagePayload,
            message::OrderCreatedPayload,
            message::ErrorPayload,
            rabbitmq_client::RabbitMQClient};
use std::error::Error;
use tracing::info;
use std::sync::Arc; // Import Arc

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load configuration from environment variables
    let config = Config::load()?;
    let amqp_addr = config.amqp_addr.clone();
    let order_created_queue = config.order_created_queue.clone();
    let user_registered_queue = config.user_registered_queue.clone();
    info!("amqp addr: {}", amqp_addr);
    info!("order created queue: {}", order_created_queue);
    info!("user registered queue: {}", user_registered_queue);
    let rabbitmq_client = Arc::new(RabbitMQClient::new(&amqp_addr, config).await?);

    let producer_client = rabbitmq_client.clone();
    tokio::spawn(async move {
        if let Err(e) = producer_task(producer_client, &order_created_queue).await {
            eprintln!("Producer task error: {}", e);
        }
    });
    let consumer_client = rabbitmq_client.clone();
    tokio::spawn(async move {
        if let Err(e) = consumer_task(consumer_client, &user_registered_queue).await {
            eprintln!("Consumer task error: {}", e);
        }
    });
    futures::future::pending::<()>().await;
    Ok(())
}
async fn producer_task(rabbitmq_client: Arc<RabbitMQClient>, queue_name : &str) -> Result<(), Box<dyn Error>> { // Corrected signature
    let message = RRMessage::new(
        RRMessageType::OrderCreated,
        RRMessagePayload::OrderCreated(OrderCreatedPayload {
            order_id: Uuid::new_v4(),
            amount: 10.0
        })
    );
    rabbitmq_client.publish(message, queue_name).await?; // Access methods through the Arc
    info!("Producer published message");

    //Simulate an error
    let order_id = Uuid::new_v4();
    let error_payload = ErrorPayload::CardDeclined {
        order_id,
        reason: "Insufficient funds".to_string(),
    };
    rabbitmq_client.send_error(error_payload, queue_name).await?;
    info!("Producer published error message");
    Ok(())
}
async fn consumer_task(rabbitmq_client: Arc<RabbitMQClient>, queue_name: &str) -> Result<(), Box<dyn Error>> {  // Corrected signature
    rabbitmq_client.consume(queue_name).await?;
    info!("Consumer started");
    // ... (Logic to consume messages using rabbitmq_client.consume) ...   // Access methods through the Arc
    Ok(())
}

// tests be;pw
mod tests {
    use lapin::{Connection, ConnectionProperties};
    use crate::env::Config;
    use std::error::Error;
    // use super::*; // imports from the current file
    #[test]
    fn test_connection_construction() -> Result<(), Box<dyn Error>> {
        let config = Config::load()?;
        let amqp_addr = config.amqp_addr.clone();
        // Mock or stub the actual connection for isolated unit testing
        let connection_properties = ConnectionProperties::default();
        let connection_result = Connection::connect(&amqp_addr, connection_properties);

        // Assert that the connection object is constructed as expected.
        assert!(connection_result.is_ok()); // Simple verification.
        Ok(())
    }
}