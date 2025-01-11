use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Represents a simple key-value store
#[derive(Clone)]  // Add this line
pub struct Database {
    data: Arc<Mutex<HashMap<String, String>>>,
}

impl Database {
    pub fn new() -> Self {
        Database {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn insert(&self, key: String, value: String) {
        let mut data = self.data.lock().unwrap();
        data.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let data = self.data.lock().unwrap();
        data.get(key).cloned()
    }
}