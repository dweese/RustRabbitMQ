// src/rabbitmq/amqp_client.rs
#![allow(dead_code)]

use lapin::{Connection, ConnectionProperties, Channel};
use thiserror::Error;
use async_trait::async_trait;
use std::fmt::Debug;

#[derive(Error, Debug)]
pub enum AmqpError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Channel error: {0}")]
    ChannelError(String),
}

// Add the trait definition here
#[async_trait]
pub trait AmqpClientTrait {
    type Error;

    fn new(amqp_uri: &str) -> Self;

    async fn ensure_connected(&mut self) -> Result<(), Self::Error>;

    async fn do_something_with_channel(&mut self) -> Result<(), Self::Error>;
}

pub struct RabbitMqClient {
    amqp_uri: String,
    connection: Option<Connection>,
    channel: Option<Channel>,
}

#[async_trait]
impl AmqpClientTrait for RabbitMqClient {
    type Error = AmqpError;

    fn new(amqp_uri: &str) -> Self {
        Self {
            amqp_uri: amqp_uri.to_string(),
            connection: None,
            channel: None,
        }
    }

    async fn ensure_connected(&mut self) -> Result<(), Self::Error> {
        // Check if we already have a working channel
        if let Some(channel) = &self.channel {
            if channel.status().connected() {
                return Ok(());
            }
        }

        // Setup new connection and channel
        let connection = Connection::connect(
            &self.amqp_uri,
            ConnectionProperties::default(),
        )
            .await
            .map_err(|e| AmqpError::ConnectionError(format!("Failed to connect: {}", e)))?;


        let channel = connection
            .create_channel()
            .await
            .map_err(|e| AmqpError::ChannelError(format!("Failed to create channel: {}", e)))?;

        // Store both connection and channel
        self.connection = Some(connection);
        self.channel = Some(channel);

        Ok(())
    }

    async fn do_something_with_channel(&mut self) -> Result<(), Self::Error> {
        // First ensure we have a valid connection & channel
        self.ensure_connected().await?;

        // // Now we can safely use the channel
        // let channel = self.channel.as_ref().unwrap();

        // Do something with channel...

        Ok(())
    }
}