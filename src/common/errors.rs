use lapin::Error as LapinError;

use serde_json::Error as SerdeError;
use std::error::Error as StdError;
use thiserror::Error;
use tokio::time::error::Elapsed;

#[derive(Debug, Error)]
pub enum RabbitError {
    #[error("RabbitMQ common error: {0}")]
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
pub type Result<T> = std::result::Result<T, RabbitError>;

// Converting from lapin errors
impl From<LapinError> for RabbitError {
    fn from(error: LapinError) -> Self {
        // Use string representation for classification
        let error_text = error.to_string();

        if error_text.contains("common") {
            RabbitError::ConnectionError(error_text)
        } else if error_text.contains("channel") {
            RabbitError::ChannelError(error_text)
        } else if error_text.contains("publish") {
            RabbitError::PublishError(error_text)
        } else if error_text.contains("consume") {
            RabbitError::ConsumeError(error_text)
        } else if error_text.contains("ack") || error_text.contains("nack") {
            RabbitError::AckError(error_text)  // Add this condition
        } else {
            RabbitError::Unknown(error_text)
        }
    }
}


impl From<Elapsed> for RabbitError {
    fn from(_: Elapsed) -> Self {
        RabbitError::TimeoutError("Connection timed out".to_string())
    }
}

impl From<String> for RabbitError {
    fn from(message: String) -> Self {
        RabbitError::Unknown(message)
    }
}

impl From<&str> for RabbitError {
    fn from(message: &str) -> Self {
        RabbitError::Unknown(message.to_string())
    }
}

// Add specific implementations for std errors:
impl From<std::env::VarError> for RabbitError {
    fn from(err: std::env::VarError) -> Self {
        RabbitError::Unknown(format!("Environment variable error: {}", err))
    }
}


impl From<std::num::ParseIntError> for RabbitError {
    fn from(err: std::num::ParseIntError) -> Self {
        RabbitError::Unknown(format!("Parse error: {}", err))
    }
}

// This allows converting any boxed error into RabbitError
impl From<Box<dyn StdError>> for RabbitError {
    fn from(error: Box<dyn StdError>) -> Self {
        RabbitError::Unknown(error.to_string())
    }
}


// Useful for converting general errors to RabbitError
// impl<E> From<E> for RabbitError
// where
//     E: std::error::Error + Send + Sync + 'static,
//     Self: Sized,
//     E: !Into<RabbitError>, // Prevent implementing `From<RabbitError>` for `RabbitError`
// {
//     fn from(error: E) -> Self {
//         RabbitError::Unknown(error.to_string())
//     }
// }
