```kotlin
# RustRabbitMQ

This repository demonstrates how to use the `lapin` crate in Rust to interact with a RabbitMQ message broker.

## Getting Started

### Prerequisites

*   **Rust:** Make sure you have Rust and Cargo installed. You can install them from [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).
*   **RabbitMQ:** Install RabbitMQ on fedora by following the official documentation, with the command `sudo dnf install rabbitmq-server`.

### Running the Example

1.  **Start RabbitMQ:**
    Start the rabbitmq service with the command `sudo systemctl start rabbitmq-server`
    Enable the rabbitmq management plugin with the command `sudo rabbitmq-plugins enable rabbitmq_management`
2. Build and run the Rust code:

    ```bash
    cargo run
    ```

This will:

*   Connect to the RabbitMQ server running on your Fedora instance.
*   publish an `order_created` message to the "order_created" queue.
*   publish an error message.
*   consume `user_registered` message from the `user_registered` queue.

### Code Structure

*   `src/message.rs`: Defines the `RRMessage` struct to represent messages.
*   `src/rabbitmq_client.rs`: Contains the `RabbitMQClient` struct with methods for interacting with RabbitMQ (connecting, publishing, consuming).
*   `src/main.rs`: Includes the main function with the example, and the `is_rabbitmq_healthy` function.
* `src/env.rs`: Contains the configuration struct, and the configuration loading logic.

### Next Steps

*   Add more features to the `RabbitMQClient`: Error handling, logging, support for different exchange types.
*   Create more complex examples: Work queues, publish/subscribe patterns.
*   Write tests: Unit tests for `rabbitmq_client`, integration tests for the example.

### Contributing

Contributions are welcome! Feel free to open issues or pull requests.
