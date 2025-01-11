mod db_trivial;
mod message;
mod rabbitmq_client;

use crate::{db_trivial::Database, message::Message, rabbitmq_client::RabbitMQClient};
use std::error::Error;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Run the RabbitMQ example

    if let Err(e) =  rabbitmq_example().await
    {
        eprintln!("MQ task  error: {}", e);
    }


        // Run the database example
    if let Err(e) =  database_example().await
    {
        eprintln!("database  error: {}", e);
    }


    Ok(())
}

async fn rabbitmq_example() -> Result<(), Box<dyn Error>> {
    // Initialize the RabbitMQ client
    let client = RabbitMQClient::new().await?;

    // Create a message
    let message = Message {
        content: "Hello, world!".to_string(),
        message_type: Some("greeting".to_string()),
    };

    // Publish the message
    if let Err(e) =    client.publish(message.clone(), "hello").await
    {
        eprintln!("publish  error: {}", e);
    }




    Ok(())
}

async fn database_example() -> Result<(), Box<dyn Error>> {
    // Initialize the database
    let database = Database::new();

    // Spawn a producer task
    let database_clone = database.clone(); // Clone the database here
    tokio::spawn(async move {
        if let Err(e) = producer_task(database_clone){
            eprintln!("Producer task error: {}", e);
        }
    });

    // Run the consumer task
    if let Err(e) = consumer_task(database){
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