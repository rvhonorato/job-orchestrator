# Kubernetes Deployment

> Work in progress!

This guide covers deploying job-orchestrator on Kubernetes.

## Configuration

### Environment Variables

**ConfigMap** (`job-orchestrator-config`):

| Variable | Description | Default |
| --- | --- | --- |
| `PORT` | Server port | `5000` |
| `DATA_PATH` | Job data directory | `/opt/data` |
| `DB_PATH` | Database path | `/opt/data/db.sqlite` |
| `MAX_AGE` | Max job age (seconds) | `172800` |
| `SERVICE_*_UPLOAD_URL` | Client upload URL | - |
| `SERVICE_*_DOWNLOAD_URL` | Client download URL | - |
| `SERVICE_*_TERMINATE_URL` | Client terminate URL | - |
| `SERVICE_*_RUNS_PER_USER` | Job limit per user | - |

**Client ConfigMap** (`client-config`):

| Variable | Description | Default |
| --- | --- | --- |
| `PORT` | Client port | `9000` |
| `DATA_PATH` | Job data directory | `/opt/data` |
| `DB_PATH` | Database path | `/opt/data/db.sqlite` |

### Persistent Storage

Two PVCs are created:

- `job-orchestrator-server-data`: 10Gi for server data
- `job-orchestrator-client-data`: 10Gi for client data

Modify in `kubernetes/persistent-volume-claims.yaml` if different sizes are needed:

```yaml
spec:
  resources:
    requests:
      storage: 5Gi
```

### Resource Limits

**Client** (in `kubernetes/client-deployment.yaml`):

```yaml
resources:
  limits:
    memory: "2Gi"
    cpu: "2"
    pids: 256
```

**Server** (in `kubernetes/server-deployment.yaml`):

```yaml
resources:
  limits:
    memory: "1Gi"
    cpu: "1"
```

## Security

### Security Context

The manifest applies the following hardening:

```yaml
securityContext:
  readOnlyRootFilesystem: true
  capabilities:
    drop:
      - ALL
  allowPrivilegeEscalation: false
```

### Network Policies

**Block external access to clients**:

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: block-client-external
  namespace: job-orchestrator
spec:
  podSelector:
    matchLabels:
      app: job-orchestrator
      component: client
  policyTypes:
    - Ingress
  ingress: []
```

**Allow only server to reach client**:

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: allow-server-to-client
  namespace: job-orchestrator
spec:
  podSelector:
    matchLabels:
      app: job-orchestrator
      component: client
  policyTypes:
    - Ingress
  ingress:
    - from:
        - podSelector:
            matchLabels:
              app: job-orchestrator
              component: server
      ports:
        - protocol: TCP
          port: 9000
```

## Ingress

wip

## Multiple Clients

wip

## Commands

wip

## Monitoring

wip

## Backup

wip

## Troubleshooting

wip

## See Also

- [Docker Deployment](./docker.md)
- [Production Deployment](./production.md)
- [Server Configuration](../configuration/server.md)
- [Client Configuration](../configuration/client.md)
