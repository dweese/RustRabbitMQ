use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RRMessage {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub message_type: RRMessageType,
    pub payload: RRMessagePayload,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum RRMessageType {
    OrderCreated,
    UserRegistered,
    Error,
    // ... other message types ...
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum RRMessagePayload {
    OrderCreated(OrderCreatedPayload),
    UserRegistered(UserRegisteredPayload),
    Error(ErrorPayload), // Add this variant
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OrderCreatedPayload {
    pub order_id: Uuid,
    pub amount: f64,
    // ... other order data ...
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserRegisteredPayload {
    pub user_id: Uuid,
    pub email: String,
    // ... other user data ...
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ErrorPayload {
    CardDeclined {
        order_id: Uuid,
        reason: String, // More specific error reason
    },
    InsufficientStock {
        product_id: Uuid,
        quantity_requested: u32,
        quantity_available: u32,
    },
    PaymentFailed {
        order_id: Uuid,
        reason: String,
    },
    InvalidOrder {
        order_id: Uuid,
        errors: Vec<String>
    }
    // ... other error conditions ...
}

impl RRMessage {
    /// Creates a new RRMessage with the current timestamp and a generated UUID.
    pub fn new(message_type: RRMessageType, payload: RRMessagePayload) -> Self {
        RRMessage {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            message_type,
            payload,
        }
    }

    /// Checks if the message is of a specific type.
    pub fn is_type(&self, message_type: RRMessageType) -> bool {
        self.message_type == message_type
    }

    /// Attempts to extract the payload as a specific type, returning an error if the types don't match.
    pub fn get_order_created_payload(&self) -> Result<OrderCreatedPayload, String> {
        if let RRMessagePayload::OrderCreated(payload) = &self.payload {
            Ok(payload.clone())
        } else {
            Err("Payload is not of type OrderCreated".to_string())
        }
    }

    /// Attempts to extract the payload as a specific type, returning an error if the types don't match.
    pub fn get_user_registered_payload(&self) -> Result<UserRegisteredPayload, String> {
        if let RRMessagePayload::UserRegistered(payload) = &self.payload {
            Ok(payload.clone())
        } else {
            Err("Payload is not of type UserRegistered".to_string())
        }
    }
    // ... other payload extraction methods (get_xxx_payload) ...
}