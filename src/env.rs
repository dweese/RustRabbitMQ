use envy::Error;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(rename = "AMQP_ADDR")]
    pub amqp_addr: String,

    #[serde(rename = "ORDER_CREATED_QUEUE")]
    pub order_created_queue: String,

    #[serde(rename = "USER_REGISTERED_QUEUE")]
    pub user_registered_queue: String,

    #[serde(default = "default_prefetch_count")]
    #[serde(rename = "RABBITMQ_PREFETCH_COUNT")]
    pub rabbitmq_prefetch_count: u16,

    #[serde(default = "default_connect_timeout_seconds")]
    #[serde(rename = "RABBITMQ_CONNECT_TIMEOUT_SECONDS")]
    pub rabbitmq_connect_timeout_seconds: u64,
}

fn default_prefetch_count() -> u16 {
    10
}

fn default_connect_timeout_seconds() -> u64 {
    10
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        envy::from_env::<Config>()
    }

    pub fn connect_timeout(&self) -> Duration {
        Duration::from_secs(self.rabbitmq_connect_timeout_seconds)
    }
}