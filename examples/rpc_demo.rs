// examples/rpc_demo.rs
use anyhow::Result;
use serde::{Deserialize, Serialize};

// Import from your crate
use rust_rabbitmq::common::request_response::{RpcClient, RpcServer};
// Or copy relevant code

#[derive(Debug, Serialize, Deserialize)]
struct CalculationRequest {
    operation: String,
    values: Vec<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CalculationResponse {
    result: f64,
}

async fn run_server() -> Result<()> {
    let uri = "amqp://guest:guest@localhost:5672";
    let exchange = "calculations";

    let mut server = RpcServer::new(uri).await?;

    server.register_handler("add", |payload: &[u8]| async move {
        let request: CalculationRequest = serde_json::from_slice(payload)?;
        let sum = request.values.iter().sum();

        let response = CalculationResponse { result: sum };
        Ok(serde_json::to_vec(&response)?)
    }).await?;

    server.register_handler("multiply", |payload: &[u8]| async move {
        let request: CalculationRequest = serde_json::from_slice(payload)?;
        let product = request.values.iter().fold(1.0, |acc, val| acc * val);

        let response = CalculationResponse { result: product };
        Ok(serde_json::to_vec(&response)?)
    }).await?;

    println!("RPC Server started. Press Ctrl+C to exit.");
    tokio::signal::ctrl_c().await?;
    Ok(())
}

async fn run_client() -> Result<()> {
    let uri = "amqp://guest:guest@localhost:5672";
    let exchange = "calculations";
    let timeout_secs = 5;

    let mut client = RpcClient::new(uri, timeout_secs).await?;

    // Addition request
    let add_request = CalculationRequest {
        operation: "add".to_string(),
        values: vec![1.5, 2.5, 3.5],
    };

    let add_response: CalculationResponse = client.call("add", &add_request).await?;
    println!("Addition result: {}", add_response.result);

    // Multiplication request
    let mul_request = CalculationRequest {
        operation: "multiply".to_string(),
        values: vec![2.0, 3.0, 4.0],
    };

    let mul_response: CalculationResponse = client.call("multiply", &mul_request).await?;
    println!("Multiplication result: {}", mul_response.result);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("server") => run_server().await,
        Some("client") => run_client().await,
        _ => {
            println!("Usage: cargo run --example rpc_demo [server|client]");
            Ok(())
        }
    }
}