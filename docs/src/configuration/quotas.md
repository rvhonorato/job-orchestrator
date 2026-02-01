# Quota System

The quota system ensures fair resource allocation by limiting concurrent jobs per user per service.

## How Quotas Work

```
User 1 submits 10 jobs for "example" service
Quota: SERVICE_EXAMPLE_RUNS_PER_USER=5

┌─────────────────────────────────────────┐
│ Jobs 1-5:  Dispatched immediately       │
│ Jobs 6-10: Remain queued                │
└─────────────────────────────────────────┘

When Job 1 completes:
┌─────────────────────────────────────────┐
│ Job 6: Now dispatched (slot available)  │
└─────────────────────────────────────────┘
```

## Configuration

Set quotas per service using environment variables:

```bash
SERVICE_<NAME>_RUNS_PER_USER=<limit>
```

### Examples

```bash
# Allow 5 concurrent jobs per user for "example" service
SERVICE_EXAMPLE_RUNS_PER_USER=5

# Allow 3 concurrent jobs per user for "haddock" service
SERVICE_HADDOCK_RUNS_PER_USER=3

# Allow 10 concurrent jobs per user for "quick" service
SERVICE_QUICK_RUNS_PER_USER=10
```

### Default Value

If not specified, the default quota is **5** concurrent jobs per user per service.

## Quota Scope

Quotas are enforced **per user, per service**:

| User | Service | Quota | Can Submit |
|------|---------|-------|------------|
| user_1 | example | 5 | Up to 5 concurrent |
| user_1 | haddock | 3 | Up to 3 concurrent |
| user_2 | example | 5 | Up to 5 concurrent (independent of user_1) |

Users don't compete with each other - each user has their own quota allocation.

## Quota States

Jobs transition through these states relative to quotas:

```
┌──────────┐     Quota      ┌────────────┐
│  Queued  │ ──Available──▶ │ Processing │
└──────────┘                └────────────┘
     │                            │
     │ Quota Exhausted            │
     ▼                            ▼
┌──────────────────┐      ┌────────────┐
│ Remains Queued   │      │ Submitted  │
│ (waits for slot) │      │ (running)  │
└──────────────────┘      └────────────┘
```

## Choosing Quota Values

### Factors to Consider

1. **Job Duration**: Longer jobs need lower quotas
2. **Resource Usage**: CPU/memory intensive jobs need lower quotas
3. **User Base**: More users may need lower per-user quotas
4. **Client Capacity**: Match quotas to available compute resources

### Guidelines

| Job Type | Suggested Quota |
|----------|-----------------|
| Quick jobs (< 1 min) | 10-20 |
| Medium jobs (1-10 min) | 5-10 |
| Long jobs (10+ min) | 2-5 |
| Resource-intensive | 1-3 |

### Example Scenarios

**Scientific Computing Platform**
```bash
# Quick validation jobs - high quota
SERVICE_VALIDATE_RUNS_PER_USER=20

# Standard analysis - medium quota
SERVICE_ANALYZE_RUNS_PER_USER=5

# Heavy simulation - low quota
SERVICE_SIMULATE_RUNS_PER_USER=2
```

**Educational Platform**
```bash
# Student exercises - moderate quota
SERVICE_EXERCISE_RUNS_PER_USER=3

# Final projects - allow more
SERVICE_PROJECT_RUNS_PER_USER=5
```

## Monitoring Quota Usage

### Check Queue Status

Jobs waiting due to quota exhaustion remain in `Queued` status:

```bash
# Check how many jobs are queued vs running
curl http://localhost:5000/swagger-ui/  # Use API explorer
```

### Server Logs

The server logs quota decisions:

```
INFO: User 1 has 5/5 jobs running for service 'example', job 123 remains queued
INFO: User 1 slot available, dispatching job 123 to service 'example'
```

## Testing Quotas

Submit multiple jobs to observe throttling:

```bash
# Submit 10 jobs with quota of 5
for i in {1..10}; do
  echo '#!/bin/bash
sleep 30
echo "Job complete" > output.txt' > run.sh

  curl -s -X POST http://localhost:5000/upload \
    -F "file=@run.sh" \
    -F "user_id=1" \
    -F "service=example" | jq -r '.status'
done
```

You'll see:
- First 5 jobs: Quickly move to `Submitted`
- Jobs 6-10: Stay in `Queued` until slots open

## Fair Scheduling

The quota system provides basic fairness:

- **Per-user isolation**: One user can't starve others
- **Per-service isolation**: Heavy service usage doesn't block other services
- **Automatic queuing**: No jobs are rejected, just delayed

### Limitations

Current limitations (improvements planned):

- No priority queues (FIFO within quota constraints)
- No global quotas (only per-user)
- No time-based quotas (e.g., jobs per hour)
- No burst allowances

## See Also

- [Server Configuration](./server.md)
- [Job Lifecycle](../architecture/job-lifecycle.md)
- [Your First Job](../getting-started/first-job.md)
