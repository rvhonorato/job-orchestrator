# Server API Endpoints

The orchestrator server exposes a REST API for job submission and retrieval.

## Base URL

```
http://localhost:5000
```

## Interactive Documentation

Swagger UI is available at:

```
http://localhost:5000/swagger
```

## Endpoints

### POST /upload

Submit a new job for processing.

**Request**

- Content-Type: `multipart/form-data`
- Max size: 400MB

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file` | file | Yes | One or more files (repeat for multiple) |
| `user_id` | integer | No | User identifier for quota tracking (default: 0) |
| `service` | string | Yes | Service name (must be configured on server) |

**Example**

```bash
curl -X POST http://localhost:5000/upload \
  -F "file=@run.sh" \
  -F "file=@input.pdb" \
  -F "user_id=1" \
  -F "service=example"
```

**Response**

```json
{
  "id": 1,
  "status": "Queued",
  "message": "Job successfully uploaded"
}
```

**Status Codes**

| Code | Description |
|------|-------------|
| `201` | Job created successfully |
| `400` | Invalid request (missing fields, invalid service) |
| `500` | Server error |

**Notes**

- At least one file must be named `run.sh`
- The `service` must match a configured service on the server
- `dest_id` is populated after the job is dispatched to a client

---

### GET /download/{id}

Get job status or download completed results.

**Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | integer | Job ID from upload response |

**Example**

```bash
# Check status (returns JSON when not completed)
curl http://localhost:5000/download/1

# Download results (returns ZIP when completed)
curl -o results.zip http://localhost:5000/download/1
```

**Response**

When the job is **not yet completed**, returns JSON:

```json
{
  "id": 1,
  "status": "Running",
  "message": ""
}
```

When the job is **completed**, returns:

- Content-Type: `application/zip`
- Body: ZIP archive of all output files

**Status Codes**

| Code | Description |
|------|-------------|
| `200` | JSON job status or ZIP file (check `Content-Type`) |
| `404` | Job not found |
| `500` | Server error |

**Notes**

- The same endpoint returns JSON or ZIP depending on job state
- Poll until `status` is `Completed`, then the next call returns the ZIP
- All files present in the job directory after `run.sh` finishes are included

---

### GET /download_partial/{id}

Retrieve current job state regardless of completion status.

Use this endpoint to debug stuck or incomplete jobs by downloading
the current workspace state as a ZIP file.

**Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | integer | Job ID from upload response |

**Example**

```bash
# Download partial results for debugging
curl -o partial_results.zip http://localhost:5000/download_partial/1
```

**Response**

- Content-Type: `application/zip`
- Body: ZIP archive containing current job directory state

**Status Codes**

| Code | Description |
|------|-------------|
| `200` | ZIP file with current job state |
| `404` | Job not found |
| `500` | Server error |

**Usage Pattern**

Poll until status is `Completed`, then save the ZIP:

```bash
while true; do
  response=$(curl -s http://localhost:5000/download/1)
  status=$(echo "$response" | jq -r '.status // empty')
  if [ -z "$status" ]; then
    # No JSON status field means we got the ZIP
    curl -o results.zip http://localhost:5000/download/1
    break
  elif [ "$status" = "Completed" ]; then
    curl -o results.zip http://localhost:5000/download/1
    break
  else
    echo "Status: $status"
    sleep 5
  fi
done
```

---

### POST /terminate/{id}

Cancel a running job.

**Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | integer | Job ID to terminate |

**Example**

```bash
curl -X POST http://localhost:5000/terminate/1
```

**Response**

On success:

```json
{
  "id": 1,
  "status": "Killed",
  "message": "job terminated"
}
```

On failure:

```json
{
  "id": 1,
  "status": "Running",
  "message": "could not terminate job"
}
```

**Status Codes**

| Code | Description |
|------|-------------|
| `200` | Job terminated successfully |
| `404` | Job not found |
| `500` | Server error or termination failed |

**Notes**

- The job must be in `Running` or `Submitted` status to be terminated
- Terminated jobs will have status `Killed`
- The server sends a termination signal to the client, which then kills the process
- If the job has already completed, the termination request will fail

---

### GET /health

Health check endpoint.

**Example**

```bash
curl http://localhost:5000/health
```

**Response**

```json
{
  "status": "healthy"
}
```

**Status Codes**

| Code | Description |
|------|-------------|
| `200` | Server is healthy |
| `500` | Server is unhealthy |

---

### GET /

Ping endpoint for basic connectivity check.

**Example**

```bash
curl http://localhost:5000/
```

**Response**

Simple acknowledgment that the server is running.

---

### GET /swagger

Interactive API documentation.

**Example**

Open in browser:

```
http://localhost:5000/swagger
```

Provides:
- Interactive API explorer
- Request/response schemas
- Try-it-out functionality

## Error Responses

All error responses follow this format:

```json
{
  "id": 0,
  "status": "Unknown",
  "message": "Description of the error"
}
```

## Rate Limiting

The server does not implement rate limiting directly. Use a reverse proxy (nginx, traefik) for rate limiting in production.

## Authentication

The server does not implement authentication directly. The `user_id` field is trusted as provided. Implement authentication at the reverse proxy layer or in your application.

## See Also

- [Client Endpoints](./client-endpoints.md)
- [Your First Job](../getting-started/first-job.md)
- [Job Lifecycle](../architecture/job-lifecycle.md)
