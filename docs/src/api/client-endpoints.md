# Client API Endpoints

The client exposes endpoints for the orchestrator server to submit jobs and retrieve results.

> **Note**: These endpoints are typically only accessed by the orchestrator server, not by end users.

## Base URL

```
http://localhost:9000
```

## Endpoints

### POST /submit

Receive a job payload from the orchestrator server.

**Request**

- Content-Type: `multipart/form-data`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file` | file | Yes | One or more job files |

**Example**

```bash
curl -X POST http://localhost:9000/submit \
  -F "file=@run.sh" \
  -F "file=@input.pdb"
```

**Response**

```json
{
  "id": 1,
  "status": "Prepared",
  "loc": "/opt/data/abc123-def456"
}
```

**Status Codes**

| Code | Description |
|------|-------------|
| `200` | Payload received successfully |
| `500` | Server error |

**Notes**

- The client stores files and creates a payload record
- Status starts as `Prepared`, waiting for the Runner task
- The `id` is returned to the server and stored as `dest_id`

---

### GET /retrieve/{id}

Retrieve results of a completed payload.

**Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | integer | Payload ID from submit response |

**Example**

```bash
curl -o results.zip http://localhost:9000/retrieve/1
```

**Response**

When the payload is **not yet completed**, returns a JSON body:

```json
{
  "id": 1,
  "status": "Running",
  "loc": "/opt/data/abc123-def456"
}
```

When the payload is **completed**, returns:

- Content-Type: `application/zip`
- Body: ZIP archive of all files in the payload directory

**Status Codes**

| Code | Description |
|------|-------------|
| `200` | JSON payload status or ZIP file (check `Content-Type`) |
| `404` | Payload not found |
| `500` | Server error |

**Notes**

- The ZIP includes all files in the working directory after `run.sh` execution
- Original input files are included unless deleted by `run.sh`
- After successful retrieval, the payload may be cleaned up

---

### POST /kill/{id}

Terminate a running payload.

**Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | integer | Payload ID to terminate |

**Example**

```bash
curl -X POST http://localhost:9000/kill/1
```

**Response**

On success (200 OK):
- Empty body

On failure:
- `404` if payload not found
- `500` if kill operation failed

**Status Codes**

| Code | Description |
|------|-------------|
| `200` | Payload terminated successfully |
| `404` | Payload not found |
| `500` | Kill operation failed |

**Notes**

- The payload must be in `Running` or `Prepared` status
- Sends SIGTERM to the process group
- The payload status will change to `Killed`
- This endpoint is called by the server's `/terminate/{id}` endpoint

---

### GET /load

Report current CPU usage.

**Example**

```bash
curl http://localhost:9000/load
```

**Response**

```json
45.2
```

Returns a float representing CPU usage percentage (0-100).

**Use Cases**

- Load-aware job distribution (planned feature)
- Monitoring client health
- Capacity planning

---

### GET /health

Health check endpoint.

**Example**

```bash
curl http://localhost:9000/health
```

**Response**

```json
{
  "status": "healthy"
}
```

---

### GET /

Ping endpoint for basic connectivity check.

**Example**

```bash
curl http://localhost:9000/
```

## Payload States

Payloads on the client go through these states:

| State | Description |
|-------|-------------|
| `Prepared` | Received from server, waiting for execution |
| `Running` | Currently executing `run.sh` |
| `Completed` | Execution finished successfully |
| `Failed` | Execution failed (non-zero exit code) |

## Security Considerations

The client API should **never** be exposed to the public internet:

- No authentication is implemented
- Arbitrary code execution via `run.sh`
- Internal service communication only

**Recommendations**:

- Use internal networks / VPCs
- Firewall rules: allow only orchestrator server IP
- Docker networks with no external exposure

## See Also

- [Server Endpoints](./server-endpoints.md)
- [Client Configuration](../configuration/client.md)
- [Server & Client Modes](../architecture/server-client.md)
