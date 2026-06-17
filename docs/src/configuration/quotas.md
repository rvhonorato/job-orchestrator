# Quota System

The quota system ensures fair resource allocation by limiting concurrent jobs
per user and per service.

## Two Independent Limits

The sender enforces two limits before dispatching a queued job:

| Variable | Scope | Default | Description |
|----------|-------|---------|-------------|
| `SERVICE_<NAME>_RUNS_PER_USER` | Per user, per service | 5 | Max jobs a single user can have active at once |
| `SERVICE_<NAME>_MAX_RUNS` | Per service (all users) | 10 | Max jobs the service can have active at once |

Both limits must have available slots for a job to be dispatched. A job that
would violate either limit remains in `Queued` status until a slot opens.

Jobs in `Processing`, `Submitted`, or `Running` status all count toward
both limits.

## How It Works

```
SERVICE_EXAMPLE_RUNS_PER_USER=3
SERVICE_EXAMPLE_MAX_RUNS=5

User 1 submits 4 jobs → 3 dispatched (per-user limit reached), 1 queued
User 2 submits 4 jobs → 2 dispatched (MAX_RUNS=5 reached), 2 queued

When one of User 1's jobs completes:
  → User 1's queued job dispatches (per-user slot free, service slot free)
```



## Quota Scope

Quotas are enforced **per user, per service** (RUNS_PER_USER) and
**per service** (MAX_RUNS):

| User | Service | RUNS_PER_USER | MAX_RUNS | Result |
|------|---------|---------------|----------|--------|
| user_1 | example | 5 | 10 | Up to 5 concurrent |
| user_2 | example | 5 | 10 | Up to 5 concurrent (independent) |
| user_1 | haddock | 2 | 4 | Up to 2 concurrent |

Users don't compete with each other for per-user slots. They do share the
`MAX_RUNS` pool for the service.

## Scheduling: Round-Robin Between Users

When multiple users have queued jobs for the same service, the sender
distributes slots in round-robin order across users. This prevents a single
user with many queued jobs from consuming all available slots ahead of others.

## Quota States

```
┌──────────┐   Both limits OK   ┌────────────┐
│  Queued  │ ─────────────────▶ │ Processing │
└──────────┘                    └────────────┘
     │                                │
     │ Either limit exceeded          ▼
     │                         ┌────────────┐
     └──────────────────────── │ Submitted  │
       (waits for a free slot) │ (running)  │
                               └────────────┘
```

## Choosing Values

### Factors to Consider

1. **Client capacity**: `MAX_RUNS` should not exceed what your client hardware can handle concurrently
2. **Job duration**: Longer jobs need lower per-user quotas
3. **Resource usage**: CPU/memory intensive jobs need lower `MAX_RUNS`
4. **User base**: More users may need lower per-user quotas

### Guidelines

| Job Type | Suggested RUNS_PER_USER | Suggested MAX_RUNS |
|----------|-------------------------|--------------------|
| Quick jobs (< 1 min) | 10-20 | 20-50 |
| Medium jobs (1-10 min) | 5-10 | 10-20 |
| Long jobs (10+ min) | 2-5 | 4-10 |
| Resource-intensive | 1-3 | 2-4 |

### Example Scenarios

**Scientific Computing Platform**
```bash
# Quick validation jobs
SERVICE_VALIDATE_RUNS_PER_USER=10
SERVICE_VALIDATE_MAX_RUNS=50

# Standard analysis
SERVICE_ANALYZE_RUNS_PER_USER=5
SERVICE_ANALYZE_MAX_RUNS=20

# Heavy simulation
SERVICE_SIMULATE_RUNS_PER_USER=2
SERVICE_SIMULATE_MAX_RUNS=4
```

**Educational Platform**
```bash
# Student exercises
SERVICE_EXERCISE_RUNS_PER_USER=3
SERVICE_EXERCISE_MAX_RUNS=30

# Final projects
SERVICE_PROJECT_RUNS_PER_USER=5
SERVICE_PROJECT_MAX_RUNS=20
```

## Testing Quotas

Submit multiple jobs to observe throttling:

```bash
# Set low limits to observe throttling
export SERVICE_EXAMPLE_RUNS_PER_USER=2
export SERVICE_EXAMPLE_MAX_RUNS=3

# Submit 5 jobs
for i in {1..5}; do
  curl -s -X POST http://localhost:5000/upload \
    -F "file=@run.sh" \
    -F "user_id=1" \
    -F "service=example" | jq -r '.status'
done
```

Expected behaviour:
- Jobs 1-2: Move to `Submitted` (per-user limit reached)
- Job 3: May dispatch (MAX_RUNS not yet reached), but per-user quota is full
- Jobs 4-5: Stay in `Queued`

## Fair Scheduling

The quota system provides basic fairness:

- **Per-user isolation**: One user can't starve others
- **Per-service cap**: Prevents overloading a single client
- **Round-robin dispatch**: Slots are shared evenly across users
- **Automatic queuing**: No jobs are rejected, just delayed

### Limitations

Current limitations (improvements planned):

- No priority queues (FIFO within quota constraints)
- No time-based quotas (e.g., jobs per hour)
- No burst allowances

## See Also

- [Server Configuration](./server.md)
- [Job Lifecycle](../architecture/job-lifecycle.md)
- [Your First Job](../getting-started/first-job.md)
