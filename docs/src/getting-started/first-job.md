# Your First Job

This guide walks you through submitting and retrieving your first job.

## Prerequisites

Make sure you have job-orchestrator running. See [Quick Start](./quick-start.md) if you haven't set it up yet.

## Understanding Jobs

A job in job-orchestrator consists of:

1. **Files**: One or more files to be processed
2. **A `run.sh` script**: The entry point that gets executed
3. **User ID**: Identifies who submitted the job (for quota tracking)
4. **Service**: Which service/backend should process this job

## Creating a Simple Job

Create a simple `run.sh` script:

```bash
cat > run.sh << 'EOF'
#!/bin/bash
echo "Hello from job-orchestrator!" > output.txt
echo "Processing complete at $(date)" >> output.txt
EOF
chmod +x run.sh
```

## Submitting the Job

Submit the job using curl:

```bash
curl -X POST http://localhost:5000/upload \
  -F "file=@run.sh" \
  -F "user_id=1" \
  -F "service=example" | jq
```

You'll receive a response like:

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

Note the `id` field - you'll need this to check status and download results.

## Checking Job Status

Use HTTP HEAD to check status without downloading:

```bash
curl -I http://localhost:5000/download/1
```

### Status Codes

| Code | Meaning |
|------|---------|
| `200` | Job completed, ready to download |
| `202` | Job queued or still running |
| `204` | Job cleaned up (expired) |
| `400` | Job invalid (user error in job) |
| `404` | Job not found |
| `410` | Job failed permanently |
| `500` | Internal server error |

## Downloading Results

Once you see status `200`, download the results:

```bash
curl -o results.zip http://localhost:5000/download/1
```

Extract and view:

```bash
unzip results.zip
cat output.txt
```

You should see:

```
Hello from job-orchestrator!
Processing complete at <timestamp>
```

## A More Complex Example

Here's a job that processes an input file:

```bash
# Create an input file
echo "sample data" > input.txt

# Create a processing script
cat > run.sh << 'EOF'
#!/bin/bash
# Count lines and words in input
wc input.txt > stats.txt
# Transform the data
tr 'a-z' 'A-Z' < input.txt > output.txt
echo "Done!" >> output.txt
EOF
chmod +x run.sh

# Submit with multiple files
curl -X POST http://localhost:5000/upload \
  -F "file=@run.sh" \
  -F "file=@input.txt" \
  -F "user_id=1" \
  -F "service=example"
```

## Important Notes

### The `run.sh` Script

- Must be named exactly `run.sh`
- Must be executable (or start with `#!/bin/bash`)
- Exit code `0` indicates success
- Non-zero exit code indicates failure
- All output files in the working directory are included in results

### File Size Limits

The default maximum upload size is 400MB. This can be configured on the server.

### Job Retention

Completed jobs are automatically cleaned up after the configured retention period (default: 48 hours). Make sure to download your results before they expire.

## Next Steps

- Learn about the [Job Lifecycle](../architecture/job-lifecycle.md)
- Configure [Quotas](../configuration/quotas.md) for your users
- Set up [Production Deployment](../deployment/production.md)
