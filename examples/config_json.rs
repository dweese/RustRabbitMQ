use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use anyhow::Result;

// Define our Config struct with Serde derive macros
#[derive(Debug, Serialize, Deserialize)]
struct Config {
    app_name: String,
    version: String,
    debug_mode: bool,
    max_connections: u32,
    allowed_hosts: Vec<String>,
    database: DatabaseConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct DatabaseConfig {
    host: String,
    port: u16,
    username: String,
    password: String,
}

fn write_config_to_file(config: &Config, path: &Path) -> Result<()> {
    // Convert the Config to JSON
    let json = serde_json::to_string_pretty(config)?;

    // Write to file
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;

    println!("Config written to {}", path.display());
    Ok(())
}

fn read_config_from_file(path: &Path) -> Result<Config> {
    // Open the file
    let mut file = File::open(path)?;

    // Read the content
    let mut content = String::new();
    file.read_to_string(&mut content)?;

    // Parse JSON back to Config
    let config: Config = serde_json::from_str(&content)?;

    Ok(config)
}

fn main() -> Result<()> {
    // Create a sample config
    let config = Config {
        app_name: "MyApp".to_string(),
        version: "1.0.0".to_string(),
        debug_mode: true,
        max_connections: 100,
        allowed_hosts: vec![
            "localhost".to_string(),
            "example.com".to_string(),
        ],
        database: DatabaseConfig {
            host: "db.example.com".to_string(),
            port: 5432,
            username: "admin".to_string(),
            password: "supersecret".to_string(),
        },
    };

    // Get a path in the current directory
    let config_path = Path::new("config.json");

    // Write config to file
    write_config_to_file(&config, config_path)?;
    println!("Original config: {:#?}", config);

    // Read config back from file
    let read_config = read_config_from_file(config_path)?;
    println!("Read config: {:#?}", read_config);

    // Verify they match
    assert_eq!(
        serde_json::to_string(&config)?,
        serde_json::to_string(&read_config)?
    );
    println!("Configs match!");

    Ok(())
}