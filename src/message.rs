use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RRMessage {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub message_type: RRMessageType,
    pub payload: RRMessagePayload
}
impl RRMessage {
    pub fn new(message_type: RRMessageType, payload: RRMessagePayload) -> Self {
        RRMessage {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            message_type,
            payload
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum RRMessageType {
    OrderCreated,
    UserRegistered,
    Error
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum RRMessagePayload {
    OrderCreated(OrderCreatedPayload),
    UserRegistered(UserRegisteredPayload),
    Error(ErrorPayload),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OrderCreatedPayload {
    pub order_id: Uuid,
    pub amount: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserRegisteredPayload {
    pub user_id: Uuid,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ErrorPayload {
    CardDeclined { order_id: Uuid, reason: String },
    InsufficientStock { product_id: String, quantity_requested: u32, quantity_available: u32 },
    PaymentFailed { order_id: Uuid, reason: String },
    InvalidOrder { order_id: Uuid, errors: Vec<String> },
}