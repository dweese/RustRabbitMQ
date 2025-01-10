mod database;
mod message;
mod rabbitmq_client;

use crate::{
    database::Database,
    message::Message,
    rabbitmq_client::RabbitMQClient, // Make sure this import is presentgit
};

use crate::rabbitmq_client::RabbitMQClient; 

use std::error::Error;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize database and RabbitMQ client
    let database = Database::new();
    let rabbitmq_client = RabbitMQClient::new().await?; // Use rabbitmq_client

    // Spawn a producer task
    let database_clone = database.clone();
    let rabbitmq_client_clone = rabbitmq_client.clone(); // Clone rabbitmq_client
    tokio::spawn(async move {
        if let Err(e) = producer_task(database_clone, rabbitmq_client_clone).await {
            // Pass the cloned rabbitmq_client to producer_task
            eprintln!("Producer task error: {}", e);
        }
    });

    // Run the consumer task
    if let Err(e) = consumer_task(database).await {;
        eprintln!("Consumer task error: {}", e);
    }

    Ok(())
}

async fn producer_task(
    database: Database,
    rabbitmq_client: RabbitMQClient, // Add rabbitmq_client as an argument
) -> Result<(), Box<dyn Error>> {
    // Simulate inserting data into the database
    database.insert("key1".to_string(), "value1".to_string());
    info!("Inserted key1 into the database");

    // Publish a message indicating the insertion
    let message = Message {
        content: "key1 inserted".to_string(),
        message_type: Some("database_update".to_string()),
    };
    rabbitmq_client
        .publish(message, "database_queue") // Use rabbitmq_client to publish
        .await?;

    Ok(())
}

async fn consumer_task(database: Database) -> Result<(), Box<dyn Error>> {
    // Simulate receiving a message and retrieving data from the database
    info!("Waiting for message to retrieve key1");
    // (In a real scenario, you would consume a message from RabbitMQ here)

    let value = database.get("key1");
    info!("Retrieved value from database: {:?}", value);

    Ok(())
}