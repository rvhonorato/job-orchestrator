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

Apply the same container hardening principles at the pod level:

- Use `securityContext` to run as non-root
- Set `readOnlyRootFilesystem: true`
- Drop capabilities with `capabilities.drop: ["ALL"]`
- Set resource limits (CPU, memory, PIDs) via `resources.limits`
- Use `NetworkPolicy` to restrict traffic between pods
- Apply Pod Security Standards (restricted profile)

## See Also

- [Docker Deployment](./docker.md)
- [Production Deployment](./production.md)
- [Server Configuration](../configuration/server.md)
- [Client Configuration](../configuration/client.md)
