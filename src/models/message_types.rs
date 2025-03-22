// src/models/message_types.rs
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaymentMessage {
    pub id: String,
    pub amount: f64,
    pub currency: String,
    pub customer_id: String,
    pub timestamp: i64,
}
