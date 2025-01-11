// In src/main.rs

mod db_trivial;
mod message;
mod rabbitmq_client;

use crate::{db_trivial::Database, message::Message, rabbitmq_client::RabbitMQClient };
use std::error::Error;
use tracing::info;
use std::io;
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::format::FmtSpan;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    // Set up tracing subscriber
    fmt() // Call fmt() to create a SubscriberBuilder
        .with_span_events(FmtSpan::CLOSE)
        .json()
        .with_writer(io::stdout) // Write to standard output
        .init();

    // Set up tracing subscriber to write to stdout
    // fmt() // Call fmt() directly
    //     .with_span_events(FmtSpan::CLOSE)
    //     .json()
    //     .with_writer(io::stdout) // Write to standard output
    //     .init();


    info!("Entered main");
    info!("before rabbitmq_example");

    // Run the RabbitMQ example
    rabbitmq_example().await?;

    info!("before database_example");

    // Run the database example
    database_example().await?;
    info!("after database_example");

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
    client.publish(message.clone(), "hello").await?;

    // Consume messages from the same queue
    client.consume("hello").await?;

    Ok(())
}

async fn database_example() -> Result<(), Box<dyn Error>> {
    // Initialize the database
    info!("entering database_example");
    let database = Database::new();

    info!("before producer task");
    // Spawn a producer task
    let database_clone = database.clone();
    tokio::spawn(async move {

        if let Err(e) = producer_task(database_clone).await {
            eprintln!("Producer task error: {}", e);
        }
    });

    // Run the consumer task
    if let Err(e) = consumer_task(database).await {
        eprintln!("Consumer task error: {}", e);
    }

    Ok(())
}

async fn producer_task(database: Database) -> Result<(), Box<dyn Error>> {
    info!("entered producer_task");
    // Simulate inserting data into the database
    database.insert("key1".to_string(), "value1".to_string());
    info!("Inserted key1 into the database");

    // Publish a message indicating the insertion
    // (This would normally involve publishing to RabbitMQ)

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