use thiserror::Error;
use lapin::{Error as LapinError}; // Import remains here at top-level

#[derive(Error, Debug)]
pub enum RabbitMQError {
    #[error("Connection error: {0}")]
    ConnectionError(#[from] LapinError),
}



// #[tokio::main]
// async fn main() -> Result<(), RabbitMQError> {
//     let _conn = connect().await?;
//     println!("Connection successful");
//     Ok(())
// }