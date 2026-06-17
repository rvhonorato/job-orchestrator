# Kubernetes Deployment

The following instructions use [Minikube](https://minikube.sigs.k8s.io/) for local testing.
See `kubernetes/` in the repository for the manifest files.

## Quick Start (Minikube)

```bash
# Build server image
docker build --target server -t job-orchestrator-server .

# Build client image
docker build --target client -t job-orchestrator-client .

# Load into Minikube
minikube image load job-orchestrator-server
minikube image load job-orchestrator-client

# Apply manifests
minikube kubectl -- apply -f kubernetes/

# Get the external IP
minikube service job-orchestrator-server --url
```

## Usage

```bash
# Set the base URL
export URL=$(minikube service job-orchestrator-server --url)

# Submit a job
curl -X POST $URL/upload \
  -F "file=@run.sh" \
  -F "user_id=1" \
  -F "service=example"

# Check status
curl $URL/download/1
```

## Security Hardening

The manifests in `kubernetes/` already apply the following hardening measures:

| Measure | Implemented |
|---------|-------------|
| Run as non-root (`runAsUser: 1000`, `runAsNonRoot: true`) | Both server and client |
| Read-only root filesystem | Both server and client |
| Drop all capabilities | Both server and client |
| Seccomp profile (`RuntimeDefault`) | Both server and client |
| CPU and memory resource limits | Both server and client |
| NetworkPolicy restricting client ingress/egress to server only | Client |

**PID limits**: Kubernetes does not expose PID limits directly in `resources.limits`. To enforce them, add a `LimitRange` object with `type: Container` and a `max.pid` entry, or configure cgroup v2 PID limits at the node level.

## See Also

- [Docker Deployment](./docker.md)
- [Production Deployment](./production.md)
- [Server Configuration](../configuration/server.md)
- [Client Configuration](../configuration/client.md)
