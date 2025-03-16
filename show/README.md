# RustRabbitMQ Presentation

This repository contains presentation materials for "RustRabbitMQ: Robust Messaging for Modern Applications" - a technical talk about combining Rust and RabbitMQ to build reliable communication systems.

## Overview

This presentation covers the implementation of a robust messaging system built with Rust and RabbitMQ, focusing on:

- The benefits of combining Rust and RabbitMQ
- Architecture design and implementation details
- Advanced messaging patterns and performance optimization
- Practical code examples and demonstrations
- Testing strategies and future development directions

## Repository Structure

- `/presentation.md` - Full presentation content with speaker notes
- `/images/` - Diagrams and visual assets for the presentation
- `/code-samples/` - Example code snippets demonstrating key concepts
- `/resources/` - Additional reference materials and links

## Setup for Demonstrations

### Prerequisites

- Rust 1.85.0 or later
- Docker (for running RabbitMQ locally)
- [Optional] Cargo-watch for development

### Running the Demo Code

1. Start a local RabbitMQ instance:
   ```bash
   docker run -d --name rabbitmq -p 5672:5672 -p 15672:15672 rabbitmq:3-management
   ```

2. Navigate to the code-samples directory:
   ```bash
   cd code-samples
   ```

3. Run the examples:
   ```bash
   # Basic publisher example
   cargo run --bin publisher
   
   # Basic consumer example
   cargo run --bin consumer
   
   # Advanced patterns example
   cargo run --bin advanced_patterns
   ```

## Key Library Dependencies

- `lapin`: Rust AMQP client library (v3.0.0-beta.1)
- `tokio`: Async runtime
- `serde`/`serde_json`: For serialization/deserialization
- `tracing`: For logging and instrumentation

## Additional Resources

- [RustRabbitMQ GitHub Repository](https://github.com/yourusername/RustRabbitMQ)
- [RabbitMQ Official Documentation](https://www.rabbitmq.com/documentation.html)
- [Lapin Documentation](https://docs.rs/lapin/)

## About the Author

[Your Name] is a [Your Position] specializing in distributed systems and message-oriented middleware. With [X years] of experience building robust communication systems, [Your Name] has contributed to multiple open-source projects in the Rust ecosystem.

## License

This presentation and associated code samples are licensed under [License Type] - see the full license in the main project repository.

# RustRabbitMQ Code Samples

This repository contains code samples demonstrating best practices for using RabbitMQ with Rust using the lapin library. These samples are designed to showcase resilient patterns and real-world usage scenarios.

## Prerequisites

- Rust 1.85.0 or later
- Docker (for running RabbitMQ)
- RabbitMQ server running on localhost:5672 (or configured via RABBITMQ_URI)

## Running RabbitMQ

```bash
docker run -d --name rabbitmq -p 5672:5672 -p 15672:15672 rabbitmq:3-management
```

## Samples Overview

1. **Connection Management** (`connection.rs`)
    - Demonstrates resilient connection handling with automatic reconnection
    - Implements exponential backoff with jitter
    - Handles connection events and statuses properly

2. **Publisher Example** (`publisher.rs`)
    - Shows how to publish messages with proper error handling
    - Implements reliable publishing patterns
    - Demonstrates exchange declaration and configuration

3. **Consumer Example** (`consumer.rs`)
    - Demonstrates message consumption with proper acknowledgment
    - Shows how to handle consumer errors gracefully
    - Implements automatic reconnection for consumers

4. **Request-Response Pattern** (`request_response.rs`)
    - Implements the synchronous-like communication pattern
    - Shows correlation ID usage and temporary reply queues
    - Demonstrates timeout handling for RPC calls

5. **Advanced Patterns** (`advanced_patterns.rs`)
    - Demonstrates Dead Letter Exchanges and TTL configuration
    - Shows implementation of batch processing for higher throughput
    - Includes error handling patterns for production use

## Running the Examples

Ensure you have a running RabbitMQ instance, then you can run each example individually:

```bash
# Compile all examples
cargo build --examples

# Run the common management example
cargo run --example common

# Run the publisher example
cargo run --example publisher

# Run the consumer example
cargo run --example consumer

# Run the request-response pattern example
cargo run --example request_response

# Run the advanced patterns example
cargo run --example advanced_patterns
```

## Configuration

All examples support configuration via environment variables or a `.env` file:
