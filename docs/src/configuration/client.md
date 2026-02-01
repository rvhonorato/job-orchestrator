# Client Configuration

The client is configured through environment variables and runs as a job executor.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `9000` | HTTP port the client listens on |

## Example Configuration

### Basic Setup

```bash
export PORT=9000
job-orchestrator client
```

### Docker Compose

```yaml
services:
  client:
    image: ghcr.io/rvhonorato/job-orchestrator:latest
    command: client
    ports:
      - "9000:9000"
    environment:
      PORT: 9000
    volumes:
      - client-data:/opt/data

volumes:
  client-data:
```

## How the Client Works

### In-Memory Database

Unlike the server, the client uses an in-memory SQLite database:

- **Fast**: No disk I/O for database operations
- **Ephemeral**: Data is lost on restart
- **Lightweight**: Minimal resource usage

This is intentional - the client only needs to track active payloads. The server maintains the authoritative job history.

### Working Directory

The client stores job files in a working directory. Each payload gets a unique subdirectory:

```
/opt/data/
├── payload-uuid-1/
│   ├── run.sh
│   ├── input.pdb
│   └── output.txt  (created by run.sh)
├── payload-uuid-2/
│   └── ...
```

### Execution Environment

When the Runner task executes a job:

1. Changes to the payload directory
2. Executes `./run.sh`
3. Captures the exit code
4. All files in the directory are included in results

### Resource Reporting

The client exposes a `/load` endpoint that reports CPU usage:

```bash
curl http://localhost:9000/load
```

Returns a float representing CPU usage percentage. This can be used by the server for load-aware scheduling (planned feature).

## Multiple Clients

You can run multiple clients for:

- **Scaling**: Handle more concurrent jobs
- **Isolation**: Different services on different machines
- **Redundancy**: Failover capability

### Same Service, Multiple Clients

Currently, configure multiple URLs in the server (round-robin planned):

```bash
# On server - points to primary client
SERVICE_EXAMPLE_UPLOAD_URL=http://client-1:9000/submit
SERVICE_EXAMPLE_DOWNLOAD_URL=http://client-1:9000/retrieve
```

### Different Services

Run specialized clients for different workloads:

```bash
# Client for general jobs
PORT=9000 job-orchestrator client

# Client for heavy computation (different machine)
PORT=9001 job-orchestrator client
```

Server configuration:
```bash
SERVICE_LIGHT_UPLOAD_URL=http://client-1:9000/submit
SERVICE_HEAVY_UPLOAD_URL=http://client-2:9001/submit
```

## Client Security

### Network Access

The client should only be accessible by the orchestrator server:

- Use internal networks / VPCs
- Firewall rules to restrict access
- Never expose client ports to the internet

### Execution Sandbox

The client executes arbitrary `run.sh` scripts. Consider:

- Running in containers with resource limits
- Using separate user accounts with minimal permissions
- Mounting only necessary directories
- Network isolation if jobs don't need internet

### Docker Resource Limits

```yaml
services:
  client:
    image: ghcr.io/rvhonorato/job-orchestrator:latest
    command: client
    deploy:
      resources:
        limits:
          cpus: '4'
          memory: 8G
        reservations:
          cpus: '1'
          memory: 1G
```

## Monitoring

### Health Check

```bash
curl http://localhost:9000/health
```

### Load Check

```bash
curl http://localhost:9000/load
```

### Container Logs

```bash
docker logs -f client-container
```

## Troubleshooting

### Client Not Receiving Jobs

1. Verify server can reach client URL
2. Check firewall rules
3. Verify service configuration on server

### Jobs Stuck in Prepared

1. Check if Runner task is running (look for logs)
2. Verify `run.sh` is executable
3. Check for permission issues in working directory

### High Memory Usage

The in-memory database grows with active payloads. If memory is high:

1. Check for stuck/zombie payloads
2. Restart the client (safe - server tracks jobs)
3. Consider more frequent cleanup

## See Also

- [Server Configuration](./server.md)
- [Server & Client Modes](../architecture/server-client.md)
- [Troubleshooting](../troubleshooting.md)
