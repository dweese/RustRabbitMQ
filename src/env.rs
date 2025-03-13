use serde::Deserialize;
use std::time::Duration;
use std::env;
use dotenv::dotenv;

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
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        dotenv().ok();
        Ok(Config {
            amqp_addr: env::var("AMQP_ADDR")?,
            order_created_queue: env::var("ORDER_CREATED_QUEUE")?,
            user_registered_queue: env::var("USER_REGISTERED_QUEUE")?,
            rabbitmq_prefetch_count: match env::var("RABBITMQ_PREFETCH_COUNT") {
                Ok(val) => val.parse()?,
                Err(_) => default_prefetch_count(),
            },
            rabbitmq_connect_timeout_seconds: match env::var("RABBITMQ_CONNECT_TIMEOUT_SECONDS") {
                Ok(val) => val.parse()?,
                Err(_) => default_connect_timeout_seconds(),
            },
        })
    }

    pub fn connect_timeout(&self) -> Duration {
        Duration::from_secs(self.rabbitmq_connect_timeout_seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_load() {
        let config_result = Config::load();
        assert!(config_result.is_ok());

        let config = config_result.unwrap();
        assert_eq!(config.amqp_addr, "amqp://user_rust:HwhYg1Lw8wUh@localhost:5672/vhost_rust");
        assert_eq!(config.order_created_queue, "order_created");
        assert_eq!(config.user_registered_queue, "user_registered");
        assert_eq!(config.rabbitmq_prefetch_count, 20);
        assert_eq!(config.rabbitmq_connect_timeout_seconds, 15);

    }

    #[test]
    fn test_config_defaults() {
        // Ensure environment variables are not set
        //Remove all variables for the test
        env::remove_var("AMQP_ADDR");
        env::remove_var("ORDER_CREATED_QUEUE");
        env::remove_var("USER_REGISTERED_QUEUE");
        env::remove_var("RABBITMQ_PREFETCH_COUNT");
        env::remove_var("RABBITMQ_CONNECT_TIMEOUT_SECONDS");

        let config_result = Config::load();
        assert!(config_result.is_ok());

        let config = config_result.unwrap();
        assert_eq!(config.rabbitmq_prefetch_count, 10);
        assert_eq!(config.rabbitmq_connect_timeout_seconds, 10);
    }

    #[test]
    fn test_config_connect_timeout() {
        let config = Config {
            amqp_addr: String::from("amqp://test:test@localhost:5672/%2f"),
            order_created_queue: String::from("test_order_created"),
            user_registered_queue: String::from("test_user_registered"),
            rabbitmq_prefetch_count: 15,
            rabbitmq_connect_timeout_seconds: 20,
        };

        assert_eq!(config.connect_timeout(), Duration::from_secs(20));
    }
}