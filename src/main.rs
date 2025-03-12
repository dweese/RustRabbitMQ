mod rabbitmq_client;
mod message;
mod env;
mod rabbitmq;
use std::error::Error; // ADDED THIS LINE
use futures_lite::future;
use uuid::Uuid;
use crate::env::Config;
use crate::{
    message::{
        RRMessage, RRMessageType, RRMessagePayload, OrderCreatedPayload, ErrorPayload,
    },
    rabbitmq_client::RabbitMQClient,
};
use tracing::info;
use std::sync::Arc;

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
    future::pending::<()>().await;
    Ok(())
}

async fn producer_task(
    rabbitmq_client: Arc<RabbitMQClient>,
    queue_name: &str,
) -> Result<(), Box<dyn Error>> {
    let message = RRMessage::new(
        RRMessageType::OrderCreated,
        RRMessagePayload::OrderCreated(OrderCreatedPayload {
            order_id: Uuid::new_v4(),
            amount: 10.0,
        }),
    );
    rabbitmq_client.publish(message, queue_name).await?;
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

async fn consumer_task(
    rabbitmq_client: Arc<RabbitMQClient>,
    queue_name: &str,
) -> Result<(), Box<dyn Error>> {
    rabbitmq_client.consume(queue_name).await?;
    info!("Consumer started");
    Ok(())
}

mod tests {
    use lapin::{Connection, ConnectionProperties};
    use crate::env::Config;
    #[test]
    fn test_connection_construction() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::load()?;
        let amqp_addr = config.amqp_addr.clone();
        let connection_properties = ConnectionProperties::default();
        let connection_result = Connection::connect(&amqp_addr, connection_properties);
        assert!(connection_result.is_ok());
        Ok(())
    }
}