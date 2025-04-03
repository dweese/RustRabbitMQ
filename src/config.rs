// src/config.rs
use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Result, Context, anyhow};
use serde::{Deserialize, Serialize};
use tracing::{info, debug};

// Configuration structures
#[derive(Debug, Serialize, Deserialize)]
pub struct RabbitConfig {
    pub connection: ConnectionConfig,
    #[serde(default)]
    pub test_settings: TestSettings,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub uri: String,
    #[serde(default = "default_timeout")]
    pub connection_timeout_ms: u64,
    #[serde(default = "default_heartbeat")]
    pub heartbeat_seconds: u16,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TestSettings {
    #[serde(default = "default_test_exchange")]
    pub test_exchange: String,
    #[serde(default = "default_queue_prefix")]
    pub test_queue_prefix: String,
    #[serde(default = "default_cleanup")]
    pub cleanup_test_resources: bool,
}

// Default values
fn default_timeout() -> u64 { 5000 }
fn default_heartbeat() -> u16 { 30 }
fn default_test_exchange() -> String { "rust_rmq_test".to_string() }
fn default_queue_prefix() -> String { "test_queue_".to_string() }
fn default_cleanup() -> bool { true }

// Configuration loading and management functions
#[allow(dead_code)]
pub fn find_config_file() -> Result<PathBuf> {
    // Check various locations
    let locations = [
        ("Current directory", Path::new("RustRabbitMQ.json")),
        ("Current directory (alternative)", Path::new("config/RustRabbitMQ.json")),
    ];

    for (location_name, path) in locations.iter() {
        if path.exists() {
            debug!("Found config file in {}: {}", location_name, path.display());
            return Ok(path.to_path_buf());
        }
    }

    // Try the user's home directory
    if let Some(home_dir) = home::home_dir() {
        let home_config = home_dir.join(".RustRabbitMQ.json");
        if home_config.exists() {
            debug!("Found config file in home directory: {}", home_config.display());
            return Ok(home_config);
        }
    }

    // If we reach here, no config file was found
    Err(anyhow!("Could not find RustRabbitMQ.json configuration file.
        Please create one in the current directory, your home directory,
        or the system configuration directory."))
}

#[allow(dead_code)]
pub fn load_config() -> Result<RabbitConfig> {
    let config_path = find_config_file()?;
    let config_content = fs::read_to_string(&config_path)
        .context(format!("Failed to read config file at {}", config_path.display()))?;

    // Parse and validate the config structure
    let config: RabbitConfig = serde_json::from_str(&config_content)
        .context("Configuration file contains invalid JSON or missing required fields")?;

    // Validate essential fields
    if config.connection.uri.is_empty() {
        return Err(anyhow!("Configuration error: connection.uri cannot be empty"));
    }

    Ok(config)
}

pub fn create_default_config_file(path: &Path) -> Result<()> {
    // Create the default configuration structure
    let default_config = RabbitConfig {
        connection: ConnectionConfig {
            uri: "amqp://guest:guest@localhost:5672/%2f".to_string(),
            connection_timeout_ms: default_timeout(),
            heartbeat_seconds: default_heartbeat(),
        },
        test_settings: TestSettings {
            test_exchange: default_test_exchange(),
            test_queue_prefix: default_queue_prefix(),
            cleanup_test_resources: default_cleanup(),
        },
    };

    // Convert to pretty JSON
    let json = serde_json::to_string_pretty(&default_config)
        .context("Failed to serialize default configuration")?;

    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .context(format!("Failed to create directory at {}", parent.display()))?;
        }
    }

    // Write the configuration file
    fs::write(path, json)
        .context(format!("Failed to write configuration to {}", path.display()))?;

    info!("Created default configuration file at: {}", path.display());

    // Print some helpful information
    println!("\nConfiguration file created at: {}", path.display());
    println!("\nThis file contains default settings for local testing.");
    println!("You should review and modify this file before running production tests.");
    println!("\nKey settings to review:");
    println!("  - connection.uri - Update if RabbitMQ is not running locally");
    println!("  - test_settings.cleanup_test_resources - Set to false during development");

    Ok(())
}