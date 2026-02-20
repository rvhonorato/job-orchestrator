# Server & Client Modes

job-orchestrator provides both server and client functionality in a single binary, configured via command-line arguments.

## Dual-Mode Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Same Binary                               │
│                                                                  │
│   ┌─────────────────────┐       ┌─────────────────────┐         │
│   │    Server Mode      │       │    Client Mode      │         │
│   │                     │       │                     │         │
│   │  - Job orchestration│       │  - Job execution    │         │
│   │  - Quota management │       │  - Result packaging │         │
│   │  - Persistent DB    │       │  - In-memory DB     │         │
│   │  - User-facing API  │       │  - Server-facing API│         │
│   └─────────────────────┘       └─────────────────────┘         │
│                                                                  │
│              job-orchestrator server    job-orchestrator client  │
└─────────────────────────────────────────────────────────────────┘
```

## Server Mode

The server is the central orchestrator that:

- Receives job submissions from users/applications
- Manages job queues and enforces quotas
- Distributes jobs to available clients
- Retrieves results and serves them to users
- Handles cleanup of expired jobs

### Starting the Server

```bash
job-orchestrator server --port 5000
```

Or with environment variables:

```bash
PORT=5000 job-orchestrator server
```

### Server Responsibilities

| Component | Purpose |
|-----------|---------|
| REST API | Handle `/upload` and `/download` requests |
| Queue Manager | Enforce per-user, per-service quotas |
| Sender Task | Dispatch jobs to clients |
| Getter Task | Retrieve completed results |
| Cleaner Task | Remove expired jobs |
| SQLite DB | Persistent job tracking |

### Server API Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/upload` | POST | Submit new job |
| `/download/:id` | GET | Get results or status |
| `/health` | GET | Health check |
| `/swagger-ui/` | GET | API documentation |

## Client Mode

The client executes jobs on behalf of the server:

- Receives job payloads from the server
- Executes the `run.sh` script
- Packages results for retrieval
- Reports system load for scheduling decisions

### Starting the Client

```bash
job-orchestrator client --port 9000
```

Or with environment variables:

```bash
PORT=9000 job-orchestrator client
```

### Client Responsibilities

| Component | Purpose |
|-----------|---------|
| REST API | Handle `/submit` and `/retrieve` requests |
| Runner Task | Execute prepared payloads |
| Bash Executor | Run `run.sh` scripts |
| In-Memory DB | Lightweight payload tracking |

### Client API Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/submit` | POST | Receive job from server |
| `/retrieve/:id` | GET | Return completed results |
| `/load` | GET | Report CPU usage |
| `/health` | GET | Health check |

## Communication Flow

```
User                Server                    Client
  │                   │                         │
  │──POST /upload────▶│                         │
  │◀─── Job ID ───────│                         │
  │                   │                         │
  │                   │──POST /submit──────────▶│
  │                   │◀─── Payload ID ─────────│
  │                   │                         │
  │                   │                    ┌────┴────┐
  │                   │                    │ Execute │
  │                   │                    │ run.sh  │
  │                   │                    └────┬────┘
  │                   │                         │
  │                   │──GET /retrieve/:id─────▶│
  │                   │◀─── results.zip ────────│
  │                   │                         │
  │──GET /download/:id▶│                         │
  │◀─── results.zip ──│                         │
```

## Deployment Patterns

### Single Machine (Development)

Both server and client on the same machine:

```bash
# Terminal 1
job-orchestrator server --port 5000

# Terminal 2
job-orchestrator client --port 9000
```

### Distributed (Production)

Server on one machine, clients on compute nodes:

```
                    ┌─────────────┐
                    │   Server    │
                    │  (port 5000)│
                    └──────┬──────┘
                           │
          ┌────────────────┼────────────────┐
          │                │                │
   ┌──────▼──────┐  ┌──────▼──────┐  ┌──────▼──────┐
   │  Client 1   │  │  Client 2   │  │  Client 3   │
   │ (compute-1) │  │ (compute-2) │  │ (compute-3) │
   └─────────────┘  └─────────────┘  └─────────────┘
```

### Multi-Service Setup

Different clients for different services:

```yaml
# Server configuration
SERVICE_EXAMPLE_UPLOAD_URL: http://client-example:9000/submit
SERVICE_EXAMPLE_DOWNLOAD_URL: http://client-example:9000/retrieve

SERVICE_HADDOCK_UPLOAD_URL: http://client-haddock:9001/submit
SERVICE_HADDOCK_DOWNLOAD_URL: http://client-haddock:9001/retrieve
```

## Database Differences

### Server Database (Persistent)

- Uses SQLite file on disk
- Survives restarts
- Stores complete job history
- Location configured via `DB_PATH`

### Client Database (In-Memory)

- SQLite in-memory database
- Cleared on restart
- Only tracks active payloads
- Lightweight and fast

## When to Scale

### Add More Clients When:

- Job queue is consistently backing up
- Execution time is the bottleneck
- You have available compute resources

### Scale Server When:

- Upload/download becomes slow
- Many concurrent users
- Database queries become slow

## See Also

- [Server Configuration](../configuration/server.md)
- [Client Configuration](../configuration/client.md)
- [Docker Deployment](../deployment/docker.md)
