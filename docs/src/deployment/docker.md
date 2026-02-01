# Docker Deployment

Docker is the recommended way to deploy job-orchestrator in production.

## Quick Start

```bash
git clone https://github.com/rvhonorato/job-orchestrator.git
cd job-orchestrator
docker compose up --build
```

This starts:

- Server on port 5000
- Example client on port 9000

## Docker Images

### Official Image

```bash
docker pull ghcr.io/rvhonorato/job-orchestrator:latest
```

### Build Locally

```bash
docker build -t job-orchestrator .
```

## Docker Compose

### Basic Setup

```yaml
version: '3.8'

services:
  server:
    image: ghcr.io/rvhonorato/job-orchestrator:latest
    command: server
    ports:
      - "5000:5000"
    environment:
      PORT: 5000
      DB_PATH: /opt/data/db.sqlite
      DATA_PATH: /opt/data
      MAX_AGE: 172800
      SERVICE_EXAMPLE_UPLOAD_URL: http://client:9000/submit
      SERVICE_EXAMPLE_DOWNLOAD_URL: http://client:9000/retrieve
      SERVICE_EXAMPLE_RUNS_PER_USER: 5
    volumes:
      - server-data:/opt/data
    depends_on:
      - client

  client:
    image: ghcr.io/rvhonorato/job-orchestrator:latest
    command: client
    environment:
      PORT: 9000
    volumes:
      - client-data:/opt/data

volumes:
  server-data:
  client-data:
```

### Production Setup

```yaml
version: '3.8'

services:
  server:
    image: ghcr.io/rvhonorato/job-orchestrator:latest
    command: server
    restart: unless-stopped
    ports:
      - "5000:5000"
    environment:
      PORT: 5000
      DB_PATH: /opt/data/db.sqlite
      DATA_PATH: /opt/data
      MAX_AGE: 172800
      SERVICE_EXAMPLE_UPLOAD_URL: http://client:9000/submit
      SERVICE_EXAMPLE_DOWNLOAD_URL: http://client:9000/retrieve
      SERVICE_EXAMPLE_RUNS_PER_USER: 5
    volumes:
      - server-data:/opt/data
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:5000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
    deploy:
      resources:
        limits:
          memory: 1G
        reservations:
          memory: 256M

  client:
    image: ghcr.io/rvhonorato/job-orchestrator:latest
    command: client
    restart: unless-stopped
    environment:
      PORT: 9000
    volumes:
      - client-data:/opt/data
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
    deploy:
      resources:
        limits:
          cpus: '4'
          memory: 8G
        reservations:
          cpus: '1'
          memory: 1G

volumes:
  server-data:
  client-data:
```

## Multiple Clients

### Scaling Horizontally

```yaml
services:
  server:
    # ... server config ...
    environment:
      SERVICE_EXAMPLE_UPLOAD_URL: http://client-1:9000/submit
      SERVICE_EXAMPLE_DOWNLOAD_URL: http://client-1:9000/retrieve

  client-1:
    image: ghcr.io/rvhonorato/job-orchestrator:latest
    command: client
    environment:
      PORT: 9000

  client-2:
    image: ghcr.io/rvhonorato/job-orchestrator:latest
    command: client
    environment:
      PORT: 9000

  client-3:
    image: ghcr.io/rvhonorato/job-orchestrator:latest
    command: client
    environment:
      PORT: 9000
```

### Multiple Services

```yaml
services:
  server:
    environment:
      # Light jobs
      SERVICE_LIGHT_UPLOAD_URL: http://client-light:9000/submit
      SERVICE_LIGHT_DOWNLOAD_URL: http://client-light:9000/retrieve
      SERVICE_LIGHT_RUNS_PER_USER: 10

      # Heavy jobs
      SERVICE_HEAVY_UPLOAD_URL: http://client-heavy:9000/submit
      SERVICE_HEAVY_DOWNLOAD_URL: http://client-heavy:9000/retrieve
      SERVICE_HEAVY_RUNS_PER_USER: 2

  client-light:
    image: ghcr.io/rvhonorato/job-orchestrator:latest
    command: client
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 2G

  client-heavy:
    image: ghcr.io/rvhonorato/job-orchestrator:latest
    command: client
    deploy:
      resources:
        limits:
          cpus: '8'
          memory: 16G
```

## Volume Management

### Persistent Storage

Always use named volumes for production:

```yaml
volumes:
  server-data:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: /data/job-orchestrator/server

  client-data:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: /data/job-orchestrator/client
```

### Backup Strategy

```bash
# Stop services (optional, for consistent backup)
docker compose stop

# Backup server data
tar -czf backup-$(date +%Y%m%d).tar.gz /data/job-orchestrator/server

# Resume services
docker compose start
```

## Networking

### Internal Network

Keep client internal:

```yaml
services:
  server:
    ports:
      - "5000:5000"  # Exposed to host
    networks:
      - internal
      - external

  client:
    # No ports exposed to host
    networks:
      - internal

networks:
  internal:
    internal: true
  external:
```

### With Reverse Proxy

```yaml
services:
  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
      - ./certs:/etc/nginx/certs:ro
    depends_on:
      - server

  server:
    # No ports exposed, accessed via nginx
    networks:
      - internal
```

## Logging

### View Logs

```bash
# All services
docker compose logs -f

# Server only
docker compose logs -f server

# Last 100 lines
docker compose logs --tail 100 server
```

### Log Rotation

```yaml
services:
  server:
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
```

## Commands

```bash
# Start
docker compose up -d

# Stop
docker compose down

# Restart
docker compose restart

# Rebuild and start
docker compose up --build -d

# View status
docker compose ps

# Shell into container
docker compose exec server /bin/sh
```

## See Also

- [Production Deployment](./production.md)
- [Server Configuration](../configuration/server.md)
- [Client Configuration](../configuration/client.md)
