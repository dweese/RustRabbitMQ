// src/models.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct RRMessage {
    pub id: String,
    pub message_type: String,
    pub payload: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub error_code: String,
    pub message: String,
    pub timestamp: String,
}
