// For examples/advanced_patterns.rs
#![allow(dead_code)]

use crate::rabbitmq::ConnectionManager;
use anyhow::Result;
use std::sync::Arc;
use std::any::{Any, TypeId};


use async_trait::async_trait;
use futures_lite::StreamExt;
use lapin::{
    options::{BasicAckOptions, BasicConsumeOptions, BasicPublishOptions},
    protocol::basic::AMQPProperties,
    Channel,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{collections::HashMap, marker::PhantomData};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

// 1. Message Trait for Type-Safe Message Handling
// This uses associated types to enforce type relationships
#[async_trait]
pub trait Message: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// The type of response this message expects
    type Response: Serialize + DeserializeOwned + Send + Sync + 'static;

    /// The routing key used for this message type
    fn routing_key() -> &'static str;

    /// Optional custom processing logic for each message type
    async fn process(self, ctx: ProcessingContext) -> Result<Self::Response>;
}

// Example message types implementing the trait
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCreated {
    pub order_id: String,
    pub customer_id: String,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderConfirmation {
    pub order_id: String,
    pub status: String,
}

#[async_trait]
impl Message for OrderCreated {
    type Response = OrderConfirmation;

    fn routing_key() -> &'static str {
        "orders.created"
    }

    async fn process(self, ctx: ProcessingContext) -> Result<Self::Response> {
        // Simulate some processing
        info!(
            "Processing order {} for customer {}",
            self.order_id, self.customer_id
        );

        // Example: Access a database client from the context
        if let Some(db_client) = ctx.get_resource::<DbClient>() {
            db_client.record_order(&self.order_id).await?;
        }

        Ok(OrderConfirmation {
            order_id: self.order_id,
            status: "confirmed".to_string(),
        })
    }
}

// 2. Processing Context with Type-Erased Resource Management
pub struct ProcessingContext {
    resources: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}



impl ProcessingContext {
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
        }
    }



    pub fn add_resource<T: 'static + Send + Sync>(&mut self, resource: T) {
        self.resources.insert(
            TypeId::of::<T>(),
            Arc::new(resource) as Arc<dyn Any + Send + Sync>,
        );
    }

    pub fn get_resource<T: 'static + Send + Sync>(&self) -> Option<&T> {
        self.resources
            .get(&TypeId::of::<T>())
            .and_then(|arc| arc.downcast_ref::<T>())
    }
}


// Mock database client for demonstration
pub struct DbClient;

impl DbClient {
    pub fn new() -> Self {
        Self {}
    }

    async fn record_order(&self, order_id: &str) -> Result<()> {
        info!("Recording order {} in database", order_id);
        Ok(())
    }
}

struct MessageRouter {
    connection_manager: ConnectionManager,
    channel: Option<Channel>,
    handlers: HashMap<String, Arc<dyn MessageHandler + Send + Sync>>,
    context: ProcessingContext,
}

// Type-erased trait for message handling
#[async_trait]
trait MessageHandler: Send + Sync {
    async fn handle(&self, delivery: &[u8], ctx: &ProcessingContext) -> Result<Vec<u8>>;
}

// Generic implementation that preserves type information
#[async_trait]
impl<M: Message> MessageHandler for MessageHandlerImpl<M> {
    async fn handle(&self, delivery: &[u8], ctx: &ProcessingContext) -> Result<Vec<u8>> {
        let message: M = serde_json::from_slice(delivery)?;
        let response = message.process(ctx.clone()).await?;
        let response_bytes = serde_json::to_vec(&response)?;
        Ok(response_bytes)
    }
}

struct MessageHandlerImpl<M: Message> {
    _phantom: PhantomData<M>,
}

impl<M: Message> MessageHandlerImpl<M> {
    fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl MessageRouter {
    pub async fn new(uri: &str) -> Result<Self> {
        Ok(Self {
            connection_manager: ConnectionManager::new(uri),
            channel: None,
            handlers: HashMap::new(),
            context: ProcessingContext::new(),
        })
    }

    // Register a resource that will be available during message processing
    pub fn register_resource<T: 'static + Send + Sync>(&mut self, resource: T) -> &mut Self {
        self.context.add_resource(resource);
        self
    }

    // Register a handler for a specific message type
    pub fn register_handler<M: Message>(&mut self) -> &mut Self {
        let handler = MessageHandlerImpl::<M>::new();
        self.handlers
            .insert(M::routing_key().to_string(), Arc::new(handler));
        self
    }

    #[instrument(skip(self))]
    pub async fn start(&mut self, exchange: &str, queue: &str) -> Result<()> {
        let channel = self.connection_manager.create_channel().await?;

        // Declare exchange and queue
        channel
            .exchange_declare(
                exchange,
                lapin::ExchangeKind::Topic,
                lapin::options::ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                Default::default(),
            )
            .await?;

        channel
            .queue_declare(
                queue,
                lapin::options::QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                Default::default(),
            )
            .await?;

        // Bind queue to all registered routing keys
        for routing_key in self.handlers.keys() {
            channel
                .queue_bind(
                    queue,
                    exchange,
                    routing_key,
                    lapin::options::QueueBindOptions::default(),
                    Default::default(),
                )
                .await?;
        }

        // Start consuming messages
        let mut consumer = channel
            .basic_consume(
                queue,
                &format!("consumer-{}", Uuid::new_v4()),
                BasicConsumeOptions::default(),
                Default::default(),
            )
            .await?;

        let handlers = self.handlers.clone();
        let ctx = self.context.clone();

        tokio::spawn(async move {
            info!("Message router started, waiting for messages");

            while let Some(delivery) = consumer.next().await {
                match delivery {
                    Ok(delivery) => {
                        let routing_key= delivery.routing_key.as_str();
                        {
                            if let Some(handler) = handlers.get(routing_key) {
                                match handler.handle(&delivery.data, &ctx).await {
                                    Ok(response) => {
                                        if let Some(reply_to) = delivery.properties.reply_to() {
                                            // Send response if reply_to is specified
                                            let mut props = AMQPProperties::default();
                                            if let Some(correlation_id) =
                                                delivery.properties.correlation_id()
                                            {
                                                props = props
                                                    .with_correlation_id(correlation_id.clone());
                                            }

                                            if let Err(e) = channel
                                                .basic_publish(
                                                    "",
                                                    reply_to.as_str(),
                                                    BasicPublishOptions::default(),
                                                    &response,
                                                    props,
                                                )
                                                .await
                                            {
                                                error!("Failed to publish response: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => error!("Error handling message: {}", e),
                                }
                            } else {
                                warn!("No handler registered for routing key: {}", routing_key);
                            }
                        }

                        // Acknowledge the message was processed
                        if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                            error!("Failed to acknowledge message: {}", e);
                        }
                    }
                    Err(e) => error!("Error receiving message: {}", e),
                }
            }
        });

        Ok(())
    }
}

// For ProcessingContext to be clonable
impl Clone for ProcessingContext {
    fn clone(&self) -> Self {
        // Clone the resources (this is a simplified implementation)
        // In a real-world app, you might use Arc for resources
        Self {
            resources: self.resources.clone(),
        }
    }
}

// Usage example in a main function
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create and configure the message router
    let mut router = MessageRouter::new("amqp://guest:guest@localhost:5672").await?;

    // Register resources and handlers
    router
        .register_resource(DbClient::new())
        .register_handler::<OrderCreated>();

    // Start the router
    router.start("messages", "order_processing").await?;

    // Keep application running
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
}
