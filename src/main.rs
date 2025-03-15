mod env;
mod errors;
mod message;
mod models;
mod rabbitmq_client;

use crate::errors::Result;

use lapin::{Connection, ConnectionProperties};

use crate::{
    message::{ErrorPayload, OrderCreatedPayload, RRMessage, RRMessagePayload, RRMessageType},
    rabbitmq_client::RabbitMQClient,
};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use uuid::Uuid;

async fn is_rabbitmq_healthy(amqp_addr: &str) -> bool {
    info!("Checking RabbitMQ health...");
    let connection_result = Connection::connect(amqp_addr, ConnectionProperties::default()).await;

    match connection_result {
        Ok(connection) => {
            info!("RabbitMQ is healthy.");
            if let Err(e) = connection.close(0, "Health check successful").await {
                eprintln!("Error closing connection: {}", e);
            }
            true
        }
        Err(e) => {
            eprintln!("RabbitMQ is not healthy: {}", e);
            false
        }
    }
}

async fn producer_task(rabbitmq_client: Arc<RabbitMQClient>, queue_name: &str) -> Result<()> {
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
    rabbitmq_client
        .send_error(error_payload, queue_name)
        .await?;
    info!("Producer published error message");
    Ok(())
}

async fn consumer_task(
    rabbitmq_client: Arc<RabbitMQClient>,
    queue_name: &str,
) -> Result<()> {

    rabbitmq_client.consume(queue_name).await?;
    info!("Consumer started");
    Ok(())

}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    // Load configuration from environment variables
    let config = env::Config::load()?;
    let amqp_addr = config.amqp_addr.clone();
    let order_created_queue = config.order_created_queue.clone();
    let user_registered_queue = config.user_registered_queue.clone();
    info!("amqp addr: {}", amqp_addr);
    info!("order created queue: {}", order_created_queue);
    info!("user registered queue: {}", user_registered_queue);

    if !is_rabbitmq_healthy(&amqp_addr).await {
        eprintln!("Exiting due to RabbitMQ health check failure.");
        return Ok(());
    }
    let rabbitmq_client = Arc::new(RabbitMQClient::new(&amqp_addr, config).await?);

    let producer_client = rabbitmq_client.clone();
    let producer_handle: JoinHandle<Result<()>> =
        tokio::spawn(async move { producer_task(producer_client, &order_created_queue).await });


    let consumer_client = rabbitmq_client.clone();
    let consumer_handle: JoinHandle<Result<()>> =
        tokio::spawn(async move { consumer_task(consumer_client, &user_registered_queue).await });


    let (producer_result, consumer_result) = tokio::join!(producer_handle, consumer_handle);

    if let Err(e) = producer_result.unwrap() {
        eprintln!("Producer task failed: {}", e);
    }
    if let Err(e) = consumer_result.unwrap() {
        eprintln!("Consumer task failed: {}", e);
    }

    Ok(())
}
