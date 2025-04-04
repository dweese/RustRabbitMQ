#!/bin/bash

# Create the necessary testing directories
mkdir -p tests/fixtures/rabbitmq/{configs,messages}
mkdir -p tests/integration
mkdir -p tests/resources

# Create a sample test configuration file
cat > tests/fixtures/rabbitmq/configs/test_config.json << 'EOF'
{
  "rabbitmq": {
    "connection": {
      "host": "localhost",
      "port": 5672,
      "username": "guest",
      "password": "guest",
      "vhost": "/"
    },
    "exchanges": [
      {
        "name": "test.exchange",
        "type": "direct",
        "durable": true,
        "auto_delete": false
      }
    ],
    "queues": [
      {
        "name": "test.queue",
        "durable": true,
        "exclusive": false,
        "auto_delete": false
      }
    ],
    "bindings": [
      {
        "exchange": "test.exchange",
        "queue": "test.queue",
        "routing_key": "test.key"
      }
    ]
  }
}
EOF

# Create a sample message fixture
cat > tests/fixtures/rabbitmq/messages/sample_message.json << 'EOF'
{
  "id": "123e4567-e89b-12d3-a456-426614174000",
  "timestamp": "2025-04-02T12:00:00Z",
  "type": "test.message",
  "payload": {
    "field1": "test value",
    "field2": 42,
    "nested": {
      "inner_field": true
    }
  }
}
EOF

# Create a sample integration test
cat > tests/integration/connection_test.rs << 'EOF'
#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::fs;

    // Replace with your actual imports
    // use crate::rabbitmq::connection::RabbitMqConnection;
    // use crate::config::Config;

    fn load_test_config() -> serde_json::Value {
        let config_path = Path::new("tests/fixtures/rabbitmq/configs/test_config.json");
        let config_str = fs::read_to_string(config_path)
            .expect("Failed to read test config file");
        serde_json::from_str(&config_str)
            .expect("Failed to parse test config JSON")
    }

    #[test]
    #[ignore] // Ignore by default as it requires a running RabbitMQ instance
    fn test_connection_establishes() {
        // This is a placeholder for your actual test
        let config = load_test_config();

        // Example test logic:
        // let connection = RabbitMqConnection::new(config);
        // assert!(connection.connect().is_ok());
    }
}
EOF

# Create a simple docker-compose file for testing
cat > tests/resources/docker-compose.yml << 'EOF'
version: '3'

services:
  rabbitmq:
    image: rabbitmq:3-management
    ports:
      - "5672:5672"
      - "15672:15672"
    environment:
      - RABBITMQ_DEFAULT_USER=guest
      - RABBITMQ_DEFAULT_PASS=guest
    volumes:
      - rabbitmq_data:/var/lib/rabbitmq

volumes:
  rabbitmq_data:
EOF

# Create a README.md file for the tests directory
cat > tests/README.md << 'EOF'
# Testing Directory Structure

This directory contains testing artifacts and integration tests for the RabbitMQ project.

## Structure

- `common/` - Common test utilities
- `fixtures/` - Test fixtures and data
  - `rabbitmq/` - RabbitMQ specific fixtures
    - `configs/` - Test configurations
    - `messages/` - Sample message payloads
- `integration/` - Integration tests
- `resources/` - Test resources like Docker configurations

## Running Tests

To run integration tests:

```bash
# Start RabbitMQ for testing
docker-compose -f tests/resources/docker-compose.yml up -d

# Run tests
cargo test -- --ignored

# Stop RabbitMQ
docker-compose -f tests/resources/docker-compose.yml down
```
EOF

echo "Testing directory structure created successfully!"
chmod +x tests/resources/docker-compose.yml
