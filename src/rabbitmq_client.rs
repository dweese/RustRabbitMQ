use lapin::{
    options::*,
    types::FieldTable,
    BasicProperties,
    Connection,
    ConnectionProperties,
    Result,
};
use futures_lite::stream::StreamExt;
use std::env;

use crate::message::Message;

pub struct RabbitMQClient {
    conn: Connection,
    channel: lapin::Channel,
}

impl RabbitMQClient {
    pub async fn new() -> Result<Self> {
        let addr = env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
        let conn = Connection::connect(&addr, ConnectionProperties::default()).await?;
        let channel = conn.create_channel().await?;

        Ok(Self { conn, channel })
    }

    pub async fn publish(&self, message: Message, queue_name: &str) -> Result<()> {
        self.channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let payload = serde_json::to_string(&message)
            .map_err(|e| lapin::Error::from(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?; // Corrected error handling

        self.channel
            .basic_publish(
                "",
                queue_name,
                BasicPublishOptions::default(),
                payload.as_bytes(), // Corrected to use as_bytes() without to_vec()
                BasicProperties::default(),
            )
            .await?
            .await?;

        println!(" [x] Sent {:?}", message);
        Ok(())
    }

pub async fn consume(&self, queue_name: &str) -> Result<()> {
    self.channel
        .queue_declare(
            queue_name,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    let mut consumer = self.channel
        .basic_consume(
            queue_name,
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    println!(" [*] Waiting for messages.");
    while let Some(delivery) = consumer.next().await {
        let delivery = delivery?;
        let message = serde_json::from_slice::<Message>(&delivery.data)
            .map_err(|e| lapin::Error::from(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        println!(" [x] Received {:?}", message);
        delivery.ack(BasicAckOptions::default()).await?;
    }

    Ok(())
}
}