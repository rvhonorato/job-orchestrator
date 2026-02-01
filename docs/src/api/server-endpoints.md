# Server API Endpoints

The orchestrator server exposes a REST API for job submission and retrieval.

## Base URL

```
http://localhost:5000
```

## Interactive Documentation

Swagger UI is available at:

```
http://localhost:5000/swagger-ui/
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
| `user_id` | integer | Yes | User identifier for quota tracking |
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
  "user_id": 1,
  "service": "example",
  "status": "Queued",
  "loc": "/opt/data/978e5a14-dc94-46ab-9507-fe0a94d688b8",
  "dest_id": ""
}
```

**Status Codes**

| Code | Description |
|------|-------------|
| `200` | Job created successfully |
| `400` | Invalid request (missing fields, invalid service) |
| `500` | Server error |

**Notes**

- At least one file must be named `run.sh`
- The `service` must match a configured service on the server
- `dest_id` is populated after the job is dispatched to a client

---

### GET /download/{id}

Download the results of a completed job.

**Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | integer | Job ID from upload response |

**Example**

```bash
curl -o results.zip http://localhost:5000/download/1
```

**Response**

- Content-Type: `application/zip`
- Body: ZIP archive containing all result files

**Status Codes**

| Code | Description |
|------|-------------|
| `200` | Job completed, returns ZIP file |
| `202` | Job queued or still running |
| `204` | Job cleaned up (results expired) |
| `400` | Job invalid (user error in job) |
| `404` | Job not found |
| `410` | Job failed permanently |
| `500` | Server error |

---

### HEAD /download/{id}

Check job status without downloading results.

**Parameters**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | integer | Job ID |

**Example**

```bash
curl -I http://localhost:5000/download/1
```

**Response**

Returns only headers, no body. Check the HTTP status code:

| Code | Meaning |
|------|---------|
| `200` | Ready to download |
| `202` | Still processing |
| `204` | Cleaned up |
| `400` | Invalid |
| `404` | Not found |
| `410` | Failed |

**Usage Pattern**

Poll until status is `200`, then download:

```bash
while true; do
  status=$(curl -s -o /dev/null -w "%{http_code}" -I http://localhost:5000/download/1)
  if [ "$status" = "200" ]; then
    curl -o results.zip http://localhost:5000/download/1
    break
  elif [ "$status" = "202" ]; then
    echo "Still processing..."
    sleep 5
  else
    echo "Error: $status"
    break
  fi
done
```

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

### GET /swagger-ui/

Interactive API documentation.

**Example**

Open in browser:

```
http://localhost:5000/swagger-ui/
```

Provides:
- Interactive API explorer
- Request/response schemas
- Try-it-out functionality

## Error Responses

All error responses follow this format:

```json
{
  "error": "Description of the error"
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
