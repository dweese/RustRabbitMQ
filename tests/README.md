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
