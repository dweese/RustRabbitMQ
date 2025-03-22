// Export each example module
pub mod advanced_patterns;
pub mod connection;
pub mod consumer;
pub mod publisher;
pub mod request_response;
pub mod type_safe_mr;
mod tokio_exec;
mod batch_processor;
mod channel_manager;
mod amqp_client;

// Re-export specific items to simplify imports elsewhere
pub use connection::ConnectionManager;
pub use type_safe_mr::{Message, ProcessingContext};
