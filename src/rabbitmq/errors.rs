// src/rabbitmq/errors.rs

use lapin::Error as LapinError;
use serde_json::Error as SerdeError;
use thiserror::Error;
use tokio::time::error::Elapsed;

#[derive(Debug, Error)]
pub enum RabbitMQError {
    #[error("RabbitMQ connection error: {0}")]
    ConnectionError(String),

    #[error("RabbitMQ channel error: {0}")]
    ChannelError(String),

    #[error("Message serialization error: {0}")]
    SerializationError(#[from] SerdeError),

    #[error("Message deserialization error: {0}")]
    DeserializationError(String),

    #[error("RabbitMQ publish error: {0}")]
    PublishError(String),

    #[error("RabbitMQ consume error: {0}")]
    ConsumeError(String),

    #[error("RabbitMQ acknowledge error: {0}")]
    AckError(String),

    #[error("Connection timeout: {0}")]
    TimeoutError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

// Custom Result type for RabbitMQ operations
pub type Result<T> = std::result::Result<T, RabbitMQError>;

impl From<Box<dyn std::error::Error>> for RabbitMQError {
    fn from(error: Box<dyn std::error::Error>) -> Self {
        RabbitMQError::Unknown(error.to_string())
    }
}


// Converting from lapin errors
impl From<LapinError> for RabbitMQError {
    fn from(error: LapinError) -> Self {
        // Use string representation for classification
        let error_text = error.to_string();

        if error_text.contains("connection") {
            RabbitMQError::ConnectionError(error_text)
        } else if error_text.contains("channel") {
            RabbitMQError::ChannelError(error_text)
        } else if error_text.contains("publish") {
            RabbitMQError::PublishError(error_text)
        } else if error_text.contains("consume") {
            RabbitMQError::ConsumeError(error_text)
        } else if error_text.contains("ack") || error_text.contains("nack") {
            RabbitMQError::AckError(error_text)
        } else {
            RabbitMQError::Unknown(error_text)
        }
    }
}

impl From<Elapsed> for RabbitMQError {
    fn from(_: Elapsed) -> Self {
        RabbitMQError::TimeoutError("Connection timed out".to_string())
    }
}

impl From<String> for RabbitMQError {
    fn from(message: String) -> Self {
        RabbitMQError::Unknown(message)
    }
}

impl From<&str> for RabbitMQError {
    fn from(message: &str) -> Self {
        RabbitMQError::Unknown(message.to_string())
    }
}