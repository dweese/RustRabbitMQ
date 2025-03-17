use std::pin::Pin;
use std::future::Future;
use lapin::ConnectionProperties;

pub struct TokioExecutor;

// Define a basic trait implementation
impl TokioExecutor {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>) {
        tokio::spawn(future);
    }
}

// Now try creating connection properties
pub fn connection_properties() -> ConnectionProperties {
    ConnectionProperties::default()
}

