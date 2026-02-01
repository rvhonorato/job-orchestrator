# Server Configuration

The orchestrator server is configured primarily through environment variables.

## Environment Variables

### Core Settings

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `5000` | HTTP port the server listens on |
| `DB_PATH` | `./db.sqlite` | Path to SQLite database file |
| `DATA_PATH` | `./data` | Directory for job file storage |
| `MAX_AGE` | `172800` | Job retention time in seconds (default: 48 hours) |

### Service Configuration

For each service you want to support, configure these variables:

| Variable Pattern | Description |
|------------------|-------------|
| `SERVICE_<NAME>_UPLOAD_URL` | Client endpoint for submitting jobs |
| `SERVICE_<NAME>_DOWNLOAD_URL` | Client endpoint for retrieving results |
| `SERVICE_<NAME>_RUNS_PER_USER` | Maximum concurrent jobs per user (default: 5) |

**Note**: `<NAME>` must be uppercase. For a service called "example", use `SERVICE_EXAMPLE_*`.

## Example Configuration

### Minimal Setup

```bash
export PORT=5000
export DB_PATH=/var/lib/job-orchestrator/db.sqlite
export DATA_PATH=/var/lib/job-orchestrator/data
export SERVICE_EXAMPLE_UPLOAD_URL=http://localhost:9000/submit
export SERVICE_EXAMPLE_DOWNLOAD_URL=http://localhost:9000/retrieve
```

### Production Setup

```bash
# Core settings
export PORT=5000
export DB_PATH=/opt/orchestrator/db.sqlite
export DATA_PATH=/opt/orchestrator/data
export MAX_AGE=172800  # 48 hours

# Example service (general purpose)
export SERVICE_EXAMPLE_UPLOAD_URL=http://compute-1:9000/submit
export SERVICE_EXAMPLE_DOWNLOAD_URL=http://compute-1:9000/retrieve
export SERVICE_EXAMPLE_RUNS_PER_USER=10

# HADDOCK service (specialized)
export SERVICE_HADDOCK_UPLOAD_URL=http://haddock-cluster:9001/submit
export SERVICE_HADDOCK_DOWNLOAD_URL=http://haddock-cluster:9001/retrieve
export SERVICE_HADDOCK_RUNS_PER_USER=3
```

### Docker Compose

```yaml
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

volumes:
  server-data:
```

## Configuration Details

### PORT

The HTTP port for the REST API. Users will connect to this port to submit jobs and download results.

```bash
PORT=5000
```

### DB_PATH

Path to the SQLite database file. The directory must exist and be writable.

```bash
DB_PATH=/var/lib/job-orchestrator/db.sqlite
```

The database is created automatically on first run. It stores:
- Job metadata (ID, user, service, status)
- Job locations and timestamps
- Client payload references

### DATA_PATH

Directory where job files are stored. Each job gets a unique subdirectory.

```bash
DATA_PATH=/var/lib/job-orchestrator/data
```

Structure:
```
/var/lib/job-orchestrator/data/
├── a1b2c3d4-e5f6-7890-abcd-ef1234567890/
│   ├── run.sh
│   ├── input.pdb
│   └── output.zip  (after completion)
├── b2c3d4e5-f6a7-8901-bcde-f12345678901/
│   └── ...
```

### MAX_AGE

How long to keep completed jobs before cleanup, in seconds.

| Value | Duration |
|-------|----------|
| `3600` | 1 hour |
| `86400` | 24 hours |
| `172800` | 48 hours (default) |
| `604800` | 1 week |

```bash
MAX_AGE=172800
```

Jobs older than this are removed by the Cleaner task.

### Service URLs

Each service needs upload and download URLs pointing to a client:

```bash
SERVICE_MYSERVICE_UPLOAD_URL=http://client-host:9000/submit
SERVICE_MYSERVICE_DOWNLOAD_URL=http://client-host:9000/retrieve
```

- **UPLOAD_URL**: Where to POST job files
- **DOWNLOAD_URL**: Where to GET results (`:id` is appended automatically)

### RUNS_PER_USER

Controls how many jobs a single user can have running simultaneously for a service:

```bash
SERVICE_EXAMPLE_RUNS_PER_USER=5
```

- Jobs exceeding the quota remain in `Queued` status
- They're automatically dispatched when slots become available
- Set higher for quick jobs, lower for resource-intensive jobs

## File Permissions

Ensure the server process has:

- **Read/Write** access to `DB_PATH` parent directory
- **Read/Write** access to `DATA_PATH` directory
- **Network access** to all configured client URLs

## Validating Configuration

Start the server and check logs:

```bash
job-orchestrator server
```

You should see:
- Port binding confirmation
- Database initialization
- Service configuration loaded

Test with a health check:

```bash
curl http://localhost:5000/health
```

## See Also

- [Client Configuration](./client.md)
- [Quota System](./quotas.md)
- [Docker Deployment](../deployment/docker.md)
