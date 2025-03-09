use lapin::{Connection, ConnectionProperties};
use tokio_amqp::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // RabbitMQ connection parameters.  Replace with your actual credentials.
    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://user:password@localhost:5672/%2f".into());

    // Establish the connection
    let conn = Connection::connect_url(&addr, ConnectionProperties::default().with_tokio()).await?;

    println!("Connected to RabbitMQ");
    conn.close(0, "Normal shutdown").await?; // Close the connection gracefully

    Ok(())
}
