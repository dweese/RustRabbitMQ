// src/processing/batch.rs
use std::error::Error;

use std::marker::PhantomData;
use std::sync::Arc;
use tokio::time::Duration;
use tokio::sync::mpsc;
use lapin::message::Delivery;

pub struct BatchProcessor<T, F> {
    batch_size: usize,
    batch_timeout: Duration,
    processor: Arc<F>,
    _phantom: PhantomData<T>,
}

impl<T, F> BatchProcessor<T, F>
where
    T: Clone + Send + 'static,
    F: Fn(Vec<T>) -> Result<(), Box<dyn Error>> + Send + Sync + 'static,
{
    pub fn new(batch_size: usize, batch_timeout: Duration, processor: F) -> Self {
        Self {
            batch_size,
            batch_timeout,
            processor: Arc::new(processor),
            _phantom: PhantomData,
        }
    }

    pub fn create_channel(
        &self,
    ) -> (
        mpsc::Sender<(T, Delivery)>,
        mpsc::Receiver<(T, Delivery)>
    ) {
        mpsc::channel(100)
    }

    pub async fn start(&self, mut rx: mpsc::Receiver<(T, Delivery)>) {
        let mut batch = Vec::with_capacity(self.batch_size);
        let mut deliveries = Vec::with_capacity(self.batch_size);

        loop {
            let timeout = tokio::time::sleep(self.batch_timeout);
            tokio::pin!(timeout);

            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Some((message, delivery)) => {
                            batch.push(message);
                            deliveries.push(delivery);

                            if batch.len() >= self.batch_size {
                                self.process_batch(&batch, &deliveries).await;
                                batch.clear();
                                deliveries.clear();
                            }
                        },
                        None => break,
                    }
                }
                _ = &mut timeout => {
                    if !batch.is_empty() {
                        self.process_batch(&batch, &deliveries).await;
                        batch.clear();
                        deliveries.clear();
                    }
                }
            }
        }
    }

    async fn process_batch(&self, batch: &Vec<T>, deliveries: &Vec<Delivery>) {
        if let Err(e) = (self.processor)(batch.clone()) {
            std::
            eprintln!("Error processing batch: {}", e);
        }

        for delivery in deliveries {
            if let Err(e) = delivery.ack(lapin::options::BasicAckOptions::default()).await {
                std::
                eprintln!("Failed to acknowledge message: {}", e);
            }
        }
    }
}
