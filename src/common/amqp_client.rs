use lapin::{Connection, ConnectionProperties, Channel};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AmqpError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Channel error: {0}")]
    ChannelError(String),
}

pub struct AmqpClient {
    amqp_uri: String,
    connection: Option<Connection>,
    channel: Option<Channel>,
}

impl AmqpClient {
    pub fn new(amqp_uri: &str) -> Self {
        Self {
            amqp_uri: amqp_uri.to_string(),
            connection: None,
            channel: None,
        }
    }

    // This method ensures we have a working connection
    async fn ensure_connected(&mut self) -> Result<(), AmqpError> {
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

    // Use this pattern for any method that needs access to the channel
    pub async fn do_something_with_channel(&mut self) -> Result<(), AmqpError> {
        // First ensure we have a valid connection & channel
        self.ensure_connected().await?;

        // Now we can safely use the channel
        let channel = self.channel.as_ref().unwrap();

        // Do something with channel...

        Ok(())
    }
}