
// src/messaging/types.rs
use tokio::sync::oneshot;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use super::error::RpcError;

pub type ResponseCallback = oneshot::Sender<Result<Vec<u8>, RpcError>>;
pub type CorrelationMap = Arc<Mutex<HashMap<String, ResponseCallback>>>;