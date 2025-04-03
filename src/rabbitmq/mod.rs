// src/rabbitmq/mod.rs
// RabbitMQ implementation for our messaging abstractions

// RabbitMQ-specific errors
pub mod errors;           // Add this line
pub mod connection;       // Implementation of connection management

// Private implementation details
mod publisher;
mod request_response;
mod type_safe_mr;
mod tokio_exec;
mod batch_processor;
mod channel_manager;
mod amqp_client;

// Re-export specific items to simplify imports elsewhere
pub use connection::ConnectionManager;
pub use type_safe_mr::Message;
pub use errors::{RabbitMQError, Result};