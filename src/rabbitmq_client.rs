// use crate::{database::Database, message::Message, rabbitmq_client::RabbitMQClient};
use std::error::Error;
use crate::{
    database::Database,
    message::Message,
    rabbitmq_client::RabbitMQClient, // Add this import
};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize database and RabbitMQ client
    let database = Database::new();
    let rabbitmq_client = RabbitMQClient::new().await?;

    // Spawn a producer task
    let database_clone = database.clone();
    let rabbitmq_client_clone = rabbitmq_client.clone();
    tokio::spawn(async move {
        if let Err(e) = producer_task(database_clone, rabbitmq_client_clone).await {
            eprintln!("Producer task error: {}", e);
        }
    });

    // Spawn a consumer task
    let rabbitmq_client_clone = rabbitmq_client.clone();
    let database_clone = database.clone();
    tokio::spawn(async move {
        if let Err(e) = consumer_task(rabbitmq_client_clone, database_clone).await {
            eprintln!("Consumer task error: {}", e);
        }
    });

    Ok(())
}

async fn producer_task(
    database: Database,
    rabbitmq_client: RabbitMQClient,
) -> Result<(), Box<dyn Error>> {
    // Simulate inserting data into the database
    database.insert("key1".to_string(), "value1".to_string());
    info!("Inserted key1 into the database");

    // Publish a message indicating the insertion
    let message = Message {
        content: "key1 inserted".to_string(),
        message_type: Some("database_update".to_string()),
    };
    rabbitmq_client.publish(message, "database_queue").await?;

    Ok(())
}

async fn consumer_task(
    rabbitmq_client: RabbitMQClient,
    database: Database,
) -> Result<(), Box<dyn Error>> {
    // Consume a message from RabbitMQ
    info!("Consuming message from database_queue");
    rabbitmq_client.consume("database_queue").await?;

    // Simulate receiving a message and retrieving data from the database
    info!("Waiting for message to retrieve key1");
    // (In a real scenario, you would process the consumed message here)

    let value = database.get("key1");
    info!("Retrieved value from database: {:?}", value);

    Ok(())
}