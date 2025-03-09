use thiserror::Error;
use lapin::{Error as LapinError}; // Import remains here at top-level

#[derive(Error, Debug)]
pub enum RabbitMQError {
    #[error("Connection error: {0}")]
    ConnectionError(#[from] LapinError),
}

pub mod direct_rabbitmq_conn {
    use lapin::Connection;
    use super::RabbitMQError; // Use statements moved inside the function
    pub async fn connect() -> Result<Connection, RabbitMQError> {
        use lapin::{Connection, ConnectionProperties}; // Use statements moved inside the function where they are used

        let addr = "amqp://guest:guest@localhost:5672/%2f";

        match Connection::connect(addr, ConnectionProperties::default()).await {
            Ok(conn) => {
                println!("Connected successfully to: {}", addr);
                Ok(conn)
            },
            Err(e) => Err(RabbitMQError::ConnectionError(e)),
        }
    }
}

use direct_rabbitmq_conn::connect;

#[tokio::main]
async fn main() -> Result<(), RabbitMQError> {
    let _conn = connect().await?;
    println!("Connection successful");
    Ok(())
}