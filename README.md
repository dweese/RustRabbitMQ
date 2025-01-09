# RustRabbitMQ

This repository demonstrates how to use the `lapin` crate in Rust to interact with a RabbitMQ message broker.

## Getting Started

### Prerequisites

* **Rust:** Make sure you have Rust and Cargo installed. You can install them from [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).
* **Docker:** Install Docker Desktop from [https://www.docker.com/get-started/](https://www.docker.com/get-started/).
* **(Optional) IntelliJ IDEA:** If you prefer using an IDE, you can install IntelliJ IDEA with the Rust plugin.

### Running the Example

1. **Start RabbitMQ:**
   ```bash
   docker run -d --hostname my-rabbit -p 5672:5672 -p 15672:15672 rabbitmq:3-management
Build and run the Rust code:
Bash

cargo run
This will:

Connect to the RabbitMQ server running in Docker.
Publish a "Hello, world!" message to the "hello" queue.
Consume the message from the queue and print it to the console.
Code Structure
src/message.rs: Defines the Message struct to represent messages.
src/rabbitmq_client.rs: Contains the RabbitMQClient struct with methods for interacting with RabbitMQ (connecting, publishing, consuming).
src/main.rs: Includes the main function with the "Hello, world!" example.
Next Steps
Add more features to the RabbitMQClient: Error handling, logging, support for different exchange types.
Create more complex examples: Work queues, publish/subscribe patterns.
Write tests: Unit tests for rabbitmq_client, integration tests for the example.
Contributing
Contributions are welcome! Feel free to open issues or pull requests.