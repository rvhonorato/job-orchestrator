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
# Required: Capture exit code for the orchestrator client
trap 'echo "$?" > .orchestrator.exit' EXIT

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
  "status": "Queued",
  "message": "Job successfully uploaded"
}
```

Note the `id` field - you'll need this to check status and download results.

## Checking Job Status

Check the job status via GET request:

```bash
curl http://localhost:5000/download/1
```

If the job is not yet completed, you'll get a JSON response:

```json
{
  "id": 1,
  "status": "Submitted",
  "message": ""
}
```

The `status` field will be one of: `Queued`, `Processing`, `Submitted`, `Running`, `Completed`, `Failed`, `Invalid`, `Cleaned`, `Unknown`, `Locked`, or `Killed`.

## Downloading Results

Once the status is `Completed`, the same endpoint returns the ZIP file:

```bash
curl -o results.zip http://localhost:5000/download/1
```

## Downloading Partial Results

To inspect the current state of a job regardless of completion status
(useful for debugging stuck jobs), use the `/download_partial/{id}` endpoint:

```bash
# Get current state of job even if incomplete
curl -o partial_results.zip http://localhost:5000/download_partial/1
````

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

## Canceling a Job

If you need to cancel a running job, use the terminate endpoint:

```bash
# Cancel job with ID 1
curl -X POST http://localhost:5000/terminate/1
```

Response on success:

```json
{
  "id": 1,
  "status": "Killed",
  "message": "job terminated"
}
```

**Notes:**
- Only jobs in `Running`, `Submitted`, or `Queued` status can be terminated
- Terminated jobs will have status `Killed`
- The server sends a termination request to the client, which kills the process
- If the job has already completed, termination will fail with a 500 error

## A More Complex Example

Here's a job that processes an input file:

```bash
# Create an input file
echo "sample data" > input.txt

# Create a processing script
cat > run.sh << 'EOF'
#!/bin/bash
# Required: Capture exit code for the orchestrator client
trap 'echo "$?" > .orchestrator.exit' EXIT

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

## Security

### Script Execution

The client executes user-submitted `run.sh` scripts with full privileges.
While a script validator rejects obviously dangerous patterns (destructive
commands, network exfiltration tools, reverse shells, privilege escalation),
it is NOT a sandbox and can be bypassed by determined actors.

**Important**: Always run the client inside a container with:
- Resource limits (CPU, memory, PIDs)
- Read-only root filesystem
- Drop all capabilities (`--cap-drop=ALL`)
- Internal network only (no external exposure)
- Non-root user

See [Production Deployment](../deployment/production.md) for details.

### Path Traversal Protection

All zip operations include path traversal protection. Paths attempting to
escape the job directory (e.g., `../../etc/passwd`) are automatically rejected.

## Important Notes

### The `run.sh` Script

- Must be named exactly `run.sh`
- Must be executable (or start with `#!/bin/bash`)
- Exit code `0` indicates success
- Non-zero exit code indicates failure
- All output files in the working directory are included in results

**Important**: For the client to properly capture the exit code of your script, it **MUST** include the following trap at the beginning:

```bash
#!/bin/bash
trap 'echo "$?" > .orchestrator.exit' EXIT
# ... rest of your script
```

This ensures that when your `run.sh` exits, it writes the exit code to `.orchestrator.exit`, which the client's Updater task reads to determine if the job completed successfully or failed.

### File Size Limits

The default maximum upload size is 400MB. This can be configured on the server.

### Path Traversal Protection

The system includes built-in protection against path traversal attacks during
zip file operations. Maliciously crafted files with paths like `../../etc/passwd`
are automatically rejected. This protection applies to both job submission
and result retrieval operations.

### User ID Parameter

Jobs can be submitted with a `user_id` for quota management:

```bash
curl -X POST http://localhost:5000/upload \
  -F "file=@run.sh" \
  -F "user_id=1" \
  -F "service=example"
```

If omitted, `user_id` defaults to 0. This allows the orchestrator to enforce
per-user quotas and track resource usage.

### Job Retention

Completed jobs are automatically cleaned up after the configured retention period (default: 48 hours). Make sure to download your results before they expire.

## Next Steps

- Learn about the [Job Lifecycle](../architecture/job-lifecycle.md)
- Configure [Quotas](../configuration/quotas.md) for your users
- Set up [Production Deployment](../deployment/production.md)
