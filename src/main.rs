mod env;
mod errors;
mod message;
mod rabbitmq_client;
mod zero_copy_deser;
pub mod common;
mod models;
mod messaging;
mod processing;


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
                eprintln!("Error closing common: {}", e);
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
    let order_id = Uuid::new_v4();
    let amount = 10.0;

    let message = RRMessage::new(
        RRMessageType::OrderCreated,
        RRMessagePayload::OrderCreated(OrderCreatedPayload { order_id, amount }),
    );

    info!(
        "➡️ PRODUCING: Order created message with order_id: {}, amount: ${:.2}",
        order_id, amount
    );

    rabbitmq_client.publish(message, queue_name).await?;
    info!(
        "✅ PRODUCED: Order created message successfully sent to queue: '{}'",
        queue_name
    );
    // Small delay for better readability in logs
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

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

async fn consumer_task(rabbitmq_client: Arc<RabbitMQClient>, queue_name: &str) -> Result<()> {
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
    let order_created_queue_producer = order_created_queue.clone(); // Clone for the producer
    let producer_handle: JoinHandle<Result<()>> =
        tokio::spawn(
            async move { producer_task(producer_client, &order_created_queue_producer).await },
        );

    let consumer_client = rabbitmq_client.clone();
    // Use the original order_created_queue here
    let consumer_handle: JoinHandle<Result<()>> =
        tokio::spawn(async move { consumer_task(consumer_client, &order_created_queue).await });

    let (producer_result, consumer_result) = tokio::join!(producer_handle, consumer_handle);

    if let Err(e) = producer_result.unwrap() {
        eprintln!("Producer task failed: {}", e);
    }
    if let Err(e) = consumer_result.unwrap() {
        eprintln!("Consumer task failed: {}", e);
    }

    Ok(())
}
