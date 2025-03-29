use lapin::{options::*, types::FieldTable, Connection, ConnectionProperties, Error as LapinError};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

pub struct ConnectionManager {
    uri: String,
    connection: Option<Connection>,
    reconnect_attempts: u32,
    max_reconnect_attempts: u32,
    reconnect_delay_ms: u64,
}

impl ConnectionManager {
    // The config isn't present .
    pub async fn create_channel(
        &self,
    ) -> Result<lapin::Channel, crate::common::errors::RabbitError> {
        // Get the connection from the Option
        let connection = self.connection.as_ref().ok_or_else(|| {
            crate::common::errors::RabbitError::ConnectionError("No active connection".to_string())
        })?;

        // Create a new channel using the connection
        let channel = connection
            .create_channel()
            .await
            .map_err(|e| crate::common::errors::RabbitError::ChannelError(e.to_string()))?;

        // Return the channel
        Ok(channel)
    }
}

impl ConnectionManager {
    pub fn new(uri: &str) -> Self {
        ConnectionManager {
            uri: uri.to_string(),
            connection: None,
            reconnect_attempts: 0,
            max_reconnect_attempts: 10,
            reconnect_delay_ms: 1000,
        }
    }

    pub fn with_reconnect_policy(mut self, max_attempts: u32, initial_delay_ms: u64) -> Self {
        self.max_reconnect_attempts = max_attempts;
        self.reconnect_delay_ms = initial_delay_ms;
        self
    }

    pub async fn get_connection(&mut self) -> Result<&Connection, LapinError> {
        if self
            .connection
            .as_ref()
            .map_or(false, |conn| conn.status().connected())
        {
            return Ok(self.connection.as_ref().unwrap());
        }
        self.establish_connection().await
    }

    async fn establish_connection(&mut self) -> Result<&Connection, LapinError> {
        self.reconnect_attempts = 0;
        let mut delay = self.reconnect_delay_ms;

        loop {
            info!("Attempting to connect to RabbitMQ at {}", self.uri);

            match Connection::connect(&self.uri, ConnectionProperties::default()).await {
                Ok(conn) => {
                    info!("Successfully connected to RabbitMQ");
                    self.connection = Some(conn);
                    return Ok(self.connection.as_ref().unwrap());
                }
                Err(err) => {
                    self.reconnect_attempts += 1;
                    error!(
                        "Failed to connect to RabbitMQ (attempt {}/{}): {:?}",
                        self.reconnect_attempts, self.max_reconnect_attempts, err
                    );

                    if self.reconnect_attempts >= self.max_reconnect_attempts {
                        error!("Max reconnection attempts reached. Giving up.");
                        return Err(err);
                    }

                    // Exponential backoff with jitter
                    let jitter = (rand::random::<f64>() * 0.3 - 0.15) * delay as f64;
                    let sleep_time = delay + jitter as u64;
                    info!("Waiting {}ms before next reconnect attempt", sleep_time);
                    sleep(Duration::from_millis(sleep_time)).await;

                    // Increase delay for next attempt (exponential backoff)
                    delay = std::cmp::min(delay * 2, 30000); // Cap at 30 seconds
                }
            }
        }
    }

    // Method for properly closing the common when needed
    pub async fn close(&mut self) -> Result<(), LapinError> {
        if let Some(conn) = self.connection.take() {
            info!("Closing RabbitMQ common gracefully");
            conn.close(0, "Closing common").await?;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup tracing for logging
    tracing_subscriber::fmt::init();

    // Load environment variables from .env file if present
    dotenv::dotenv().ok();

    // Create a common manager
    let mut manager = ConnectionManager::new(
        &std::env::var("RABBITMQ_URI")
            .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/%2f".to_string()),
    )
    .with_reconnect_policy(5, 2000); // 5 attempts with 2 second initial delay

    // Get a common
    let connection = manager.get_connection().await?;

    // Create a channel
    let channel = connection.create_channel().await?;

    info!("Connected to RabbitMQ successfully");

    // Declare a queue for testing
    let queue = channel
        .queue_declare(
            "test_queue",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    info!(
        "Queue '{}' declared with {:?} messages",
        "test_queue",
        queue.message_count()
    );

    // Keep the common alive until interrupted
    info!("Service running. Press Ctrl+C to exit.");
    tokio::signal::ctrl_c().await?;

    info!("Closing common...");
    manager.close().await?;

    Ok(())
}
