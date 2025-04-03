# RustRabbitMQ Library Architecture

## Overview

RustRabbitMQ is a lightweight, ergonomic wrapper around the `lapin` crate that provides a clean, idiomatic Rust interface for working with RabbitMQ. The library follows a two-layer architecture that separates abstract messaging interfaces from their concrete implementations.

## Core Architecture

```
src/
├── common/           # Abstract interfaces (traits)
│   ├── connection.rs # Connection management interface
│   ├── publisher.rs  # Message publishing interface
│   ├── consumer.rs   # Message consuming interface
│   └── mod.rs        # Module exports
├── rabbitmq/         # RabbitMQ/lapin implementation
│   ├── connection.rs # RabbitMQ connection implementation
│   ├── publisher.rs  # RabbitMQ publisher implementation
│   ├── consumer.rs   # RabbitMQ consumer implementation
│   ├── error.rs      # RabbitMQ-specific errors
│   └── mod.rs        # Module exports
└── lib.rs            # Library entry point
```

## Key Components

### Interface Layer (`common/`)

- **ConnectionManager**: Handles connection lifecycle (connect, disconnect, status)
- **Publisher**: Supports basic publishing, publisher confirms, and request-reply pattern
- **Consumer**: Provides message consumption, acknowledgment, and rejection capabilities

### Implementation Layer (`rabbitmq/`)

- **RabbitMQConnection**: Implements connection management with automatic reconnection
- **RabbitMQPublisher**: Provides message publishing with various delivery guarantees
- **RabbitMQConsumer**: Handles message consumption with deserialization and error handling

## Design Philosophy

1. **Simplicity**: Clean interfaces that hide the complexity of AMQP
2. **Rust Idioms**: Leverages traits, generics, and the Result type
3. **Async First**: Built around Tokio and async/await
4. **Error Handling**: Comprehensive error handling with context
5. **Type Safety**: Strong typing for messages using Serde

## Features

- Connection management with automatic reconnection
- Simple message publishing
- Publish with confirmation
- Request-reply pattern with correlation IDs
- Message consumption with automatic deserialization
- Proper error handling and propagation
- Structured logging integration

## Usage Example

```rust
// Create connection
let mut connection = RabbitMQConnection::new();
connection.connect("amqp://guest:guest@localhost:5672").await?;

// Create channel
let channel = connection.create_channel().await?;

// Create publisher
let publisher = RabbitMQPublisher::new(channel.clone());

// Publish a message
let order = OrderCreated { /* ... */ };
publisher.publish("", "orders", &order).await?;

// Create consumer
let consumer = RabbitMQConsumer::new(channel);

// Start consuming messages
consumer.consume("orders", |order: OrderData| async move {
    // Process the order
    Ok(())
}).await?;
```

This library provides a foundation for reliable messaging with RabbitMQ while maintaining idiomatic Rust code that's easy to understand and extend.

User:
Save me some time and emit that, in Markdown format

