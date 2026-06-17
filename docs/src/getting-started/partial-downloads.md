# Partial Downloads and Debugging

This guide demonstrates how to use the `/download_partial` and `/retrieve_partial` endpoints to debug stuck or incomplete jobs.

## Use Cases

Use partial download endpoints when:
- A job appears stuck in `Submitted` or `Running` status
- You need to inspect the current working directory
- Debugging why a job isn't progressing
- Verifying intermediate outputs of long-running jobs

## Server-Side: /download_partial/{id}

### Example: Checking Incomplete Job

```bash
# Download current state of job ID 1
curl -o partial_results.zip http://localhost:5000/download_partial/1

# Extract and inspect contents
unzip -l partial_results.zip
unzip partial_results.zip

# View specific files
cat intermediate_output.txt
```

## Client-Side: /retrieve_partial/{id}

This endpoint is called internally by the server's `/download_partial` endpoint.
It provides the same functionality at the client level.

### Example (Client Developer)

```bash
# Get partial results directly from client
curl -o partial_results.zip http://localhost:9000/retrieve_partial/1
```

## Comparison: Full vs Partial Download

| Endpoint | Returns | Use Case |
|----------|---------|----------|
| `/download/{id}` | Final results only | Normal completion |
| `/download_partial/{id}` | Current state always | Debugging, monitoring |

## Combined Example Script

```bash
#!/bin/bash

SERVER_URL="http://localhost:5000"

# Submit job and parse the returned ID
echo "Submitting job..."
RESPONSE=$(curl -s -X POST "${SERVER_URL}/upload" \
  -F "file=@run.sh" \
  -F "user_id=1" \
  -F "service=example")
echo "$RESPONSE"
JOB_ID=$(echo "$RESPONSE" | jq -r '.id')

if [ -z "$JOB_ID" ] || [ "$JOB_ID" = "null" ]; then
  echo "Failed to get job ID from response"
  exit 1
fi

echo "Job ID: $JOB_ID"

# Poll until complete, saving partial snapshots for inspection
echo "Monitoring job..."
for i in $(seq 1 10); do
  sleep 10
  STATUS=$(curl -s "${SERVER_URL}/download/${JOB_ID}" | jq -r '.status // empty')
  if [ -z "$STATUS" ]; then
    echo "Job completed, downloading results..."
    curl -s -o final_results.zip "${SERVER_URL}/download/${JOB_ID}"
    exit 0
  fi
  echo "Check $i: status=$STATUS — saving partial snapshot..."
  curl -s -o "partial_${i}.zip" "${SERVER_URL}/download_partial/${JOB_ID}"
  unzip -l "partial_${i}.zip" 2>/dev/null | head -20
done

echo "Job did not complete within the polling window"
exit 1
```

## Limitations

- Partial downloads may be empty if the job has not written any files yet
- Partial downloads of running jobs may be incomplete or inconsistent
- Use partial downloads for debugging, not for production result retrieval

## Security Notes

- All partial downloads include path traversal protection
- Files with paths attempting to escape the job directory are rejected
- See Security Documentation for details on protection mechanisms
