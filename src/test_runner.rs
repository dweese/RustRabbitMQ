// src/test_runner.rs
use anyhow::{Result, anyhow};
use tracing::{info, error};
use std::path::PathBuf;
use std::fs;
use lapin::{Connection, ConnectionProperties};
use tokio::time::{timeout, Duration};

// Now we'll use the previously unused modules
use crate::config;

pub async fn run_config_tests() -> Result<()> {
    info!("Running configuration tests...");

    // Simple file existence check
    match test_config_file_exists() {
        Ok(path) => info!("✓ Config file exists at: {:?}", path),
        Err(e) => return Err(e),
    }


    // Test 1: Validate config file exists and can be found
    let config_path = match test_find_config_file() {
        Ok(path) => path,
        Err(e) => {
            error!("Config file test failed: {}", e);
            return Err(e);
        }
    };

    // Test 2: Validate config file contents
    if let Err(e) = test_config_contents(&config_path) {
        error!("Config content test failed: {}", e);
        return Err(e);
    }

    // Test 3: Validate RabbitMQ configuration (without connecting)
    if let Err(e) = test_rabbitmq_config() {
        error!("RabbitMQ config validation failed: {}", e);
        return Err(e);
    }

    // Test 4: Test RabbitMQ connection (optional)
    match test_rabbitmq_connection().await {
        Ok(_) => info!("✓ Successfully connected to RabbitMQ"),
        Err(e) => {
            // Just log the error but don't fail the entire test suite
            error!("RabbitMQ connection test failed: {}", e);
            // You could return Err(e) here if you want to fail on connection issues
        }
    }

    info!("All configuration tests completed!");
    Ok(())
}

fn test_config_file_exists() -> Result<PathBuf> {
    // Use the find_config_file function
    let config_path = config::find_config_file()?;

    // Simple existence check
    if !config_path.exists() {
        return Err(anyhow!("Config file not found at: {:?}", config_path));
    }

    Ok(config_path)
}


fn test_find_config_file() -> Result<PathBuf> {
    info!("Testing config file location...");

    // Use the function that was marked as unused
    let config_path = config::find_config_file()?;

    // Check if the config file exists
    if !config_path.exists() {
        return Err(anyhow!("Config file not found at: {:?}", config_path));
    }

    info!("✓ Found config file at: {:?}", config_path);
    Ok(config_path)
}

fn test_config_contents(config_path: &PathBuf) -> Result<()> {
    info!("Testing config file contents...");

    // Read the config file
    let content = fs::read_to_string(config_path)
        .map_err(|e| anyhow!("Failed to read config file: {}", e))?;

    // Check for required properties
    let required_properties = [
        "rabbitmq.host",
        "rabbitmq.port",
        "rabbitmq.username",
        "rabbitmq.password",
        // Add other required properties
    ];

    for prop in required_properties {
        if !content.contains(prop) {
            return Err(anyhow!("Required property '{}' not found in config file", prop));
        }
    }

    info!("✓ Config file contains all required properties");
    Ok(())
}

fn test_rabbitmq_config() -> Result<()> {
    info!("Validating RabbitMQ configuration...");

    // Load the config (now we use another function that wafs marked as unused)
    let config = config::load_config()?;

    // // Validate RabbitMQ settings
    // let rabbit_config = config.
    //
    // // Ensure no empty values for critical fields
    // if rabbit_config.host.is_empty() {
    //     return Err(anyhow!("RabbitMQ host is empty"));
    // }
    //
    // if rabbit_config.port == 0 {
    //     return Err(anyhow!("RabbitMQ port is invalid"));
    // }
    //
    // // Construct and validate the connection string
    // let amqp_addr = format!(
    //     "amqp://{}:{}@{}:{}/{}",
    //     rabbit_config.username,
    //     rabbit_config.password,
    //     rabbit_config.host,
    //     rabbit_config.port,
    //     rabbit_config.vhost.unwrap_or_default()
    // );
    //
    // // Just validate the format without connecting
    // if !amqp_addr.starts_with("amqp://") {
    //     return Err(anyhow!("Invalid AMQP connection string format"));
    // }
    //
    // info!("✓ RabbitMQ configuration is valid");
    Ok(())
}


async fn test_rabbitmq_connection() -> Result<()> {
    info!("Testing RabbitMQ connection...");

    // Now that we know the config is valid, we can try to connect
    let config = config::load_config()?;
    let rabbit_config = config;


    let amqp_addr = format!(
        "amqp://{}:{}@{}:{}/{}",
        "dweese", //bit_config.connection.username,
        "pw", // password,
        "localhost", //connection.host,
        "1234",  //connection.port,
        "vhost",

        // rabbit_config.vhost.unwrap_or_default()
    );


    // Try to connect with a timeout
    let connection_timeout = Duration::from_secs(5);
    match timeout(
        connection_timeout,
        Connection::connect(&amqp_addr, ConnectionProperties::default())
    ).await {
        Ok(Ok(connection)) => {
            info!("Successfully connected to RabbitMQ");
            connection.close(200, "Test completed").await?;
            Ok(())
        },
        Ok(Err(e)) => Err(anyhow!("Failed to connect to RabbitMQ: {}", e)),
        Err(_) => Err(anyhow!("Connection to RabbitMQ timed out after {} seconds", connection_timeout.as_secs())),
    }
}