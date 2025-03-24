// src/messaging/error.rs
use thiserror::Error;
use lapin::Error as LapinError;
use std::time::Duration;
use tokio::sync::oneshot;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Error, Debug)]
pub enum RpcError {
    #[error("Failed to connect to RabbitMQ: {0}")]
    ConnectionError(#[from] LapinError),

    #[error("Failed to serialize/deserialize message: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Request timeout after {0:?}")]
    Timeout(Duration),

    #[error("Response channel closed")]
    ResponseChannelClosed,
}

pub type ResponseCallback = oneshot::Sender<Result<Vec<u8>, RpcError>>;
pub type CorrelationMap = Arc<Mutex<HashMap<String, ResponseCallback>>>;