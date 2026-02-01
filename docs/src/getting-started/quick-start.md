# Quick Start

The fastest way to get job-orchestrator running is with Docker Compose.

## Running with Docker Compose

```bash
git clone https://github.com/rvhonorato/job-orchestrator.git
cd job-orchestrator
docker compose up --build
```

This starts:

- **Orchestrator server** on port 5000
- **Example client** on port 9000

## Verify It's Running

Check the server is responding:

```bash
curl http://localhost:5000/health
```

You should receive a health status response.

## Access the API Documentation

Open your browser and navigate to:

```
http://localhost:5000/swagger-ui/
```

This provides interactive API documentation where you can explore and test all endpoints.

## What's Next?

Now that you have job-orchestrator running, proceed to [Your First Job](./first-job.md) to learn how to submit and retrieve jobs.

## Stopping the Services

To stop the services:

```bash
docker compose down
```

To stop and remove all data (volumes):

```bash
docker compose down -v
```
