use crate::database::Database;
use crate::message::Message;
use crate::rabbitmq_client::RabbitMQClient;
use std::error::Error;
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

    // Run the consumer task
    if let Err(e) = consumer_task(database).await {
        eprintln!("Consumer task error: {}", e);
    }

    Ok(())
}

async fn database_example() -> Result<(), Box<dyn Error>> {
    // Initialize the database
    let database = Database::new();

    // Spawn a producer task
    let database_clone = database.clone(); // Clone the database here
    tokio::spawn(async move {
        if let Err(e) = producer_task(database_clone) {
            eprintln!("Producer task error: {}", e);
        }
    });

    // Run the consumer task
    if let Err(e) = consumer_task(database) {
        eprintln!("Consumer task error: {}", e);
    }

    Ok(())
}

fn producer_task(database: Database) -> Result<(), Box<dyn Error>> {
    // Simulate inserting data into the database
    database.insert("key1".to_string(), "value1".to_string());
    info!("Inserted key1 into the database");

    // Publish a message indicating the insertion
    // (This would normally involve publishing to RabbitMQ)

    Ok(())
}

fn consumer_task(database: Database) -> Result<(), Box<dyn Error>> {
    // Simulate receiving a message and retrieving data from the database
    info!("Waiting for message to retrieve key1");
    // (In a real scenario, you would consume a message from RabbitMQ here)

    let value = database.get("key1");
    info!("Retrieved value from database: {:?}", value);

    Ok(())
}
