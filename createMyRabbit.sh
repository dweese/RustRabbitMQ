# Stop the container
docker stop 56249e2aef35

# Remove the container (this will also remove the anonymous volume)
docker rm 56249e2aef35

# Create a named volume
docker volume create rabbitmq_data

# Run a new container with the named volume
docker run -d \
    --hostname my-rabbit \
    -p 5672:5672 \
    -p 15672:15672 \
    -v rabbitmq_data:/var/lib/rabbitmq \
    rabbitmq:3-management