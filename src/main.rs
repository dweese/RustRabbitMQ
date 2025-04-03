// src/main.rs
use anyhow::Result;
use std::path::Path;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};
use tracing_subscriber::prelude::*;

// Internal modules declarations (using mod, not use)
mod config;
mod env;
mod rabbitmq_client;
pub mod rabbitmq;
pub mod common;
mod models;
mod messaging;
mod processing;
mod utils;

mod test_runner;  // This declares the test_runner module

#[tokio::main]
async fn main() -> Result<()> {
    // Set up logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    info!("RustRabbitMQ test framework starting up");

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "--generate-config" => {
                let config_path = if args.len() > 2 {
                    Path::new(&args[2])
                } else {
                    Path::new("RustRabbitMQ.json")
                };

                config::create_default_config_file(config_path)?;
                return Ok(());
            },
            "--test" => {
                print_usage();
                return Ok(());
            },
            "--elp" => {
                print_usage();
                return Ok(());
            },
            _ => {
                println!("Unknown argument: {}", args[1]);
                print_usage();
                return Ok(());
            }
        }
    }

    // Run all tests using the function from the test_runner module
    test_runner::run_config_tests().await?;

    info!("All tests completed successfully");
    Ok(())
}

fn print_usage() {
    println!("RustRabbitMQ Test Framework");
    println!("\nUsage:");
    println!("  cargo run                      - Run all tests");
    println!("  cargo run -- --generate-config - Create default config file");
    println!("  cargo run -- --generate-config <path> - Create config at specified path");
    println!("  cargo run -- --help            - Show this help message");
}