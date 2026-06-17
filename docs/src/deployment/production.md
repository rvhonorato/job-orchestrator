# Production Deployment

This guide covers best practices for deploying job-orchestrator in production environments.

## Architecture Recommendations

### Minimum Setup

```
┌─────────────┐     ┌─────────────┐
│   Server    │────▶│   Client    │
│ (1 instance)│     │ (1 instance)│
└─────────────┘     └─────────────┘
```

### Recommended Setup

```
                    ┌──────────────┐
                    │ Load Balancer│
                    │   (nginx)    │
                    └──────┬───────┘
                           │
                    ┌──────▼───────┐
                    │    Server    │
                    │  (1 instance)│
                    └──────┬───────┘
                           │
          ┌────────────────┼────────────────┐
          │                │                │
   ┌──────▼──────┐  ┌──────▼──────┐  ┌──────▼──────┐
   │  Client 1   │  │  Client 2   │  │  Client 3   │
   └─────────────┘  └─────────────┘  └─────────────┘
```

## Security

### Script Validation

The client validates `run.sh` before execution. A script is rejected if it:

- **Exceeds 20 MiB** in size
- **Is not valid UTF-8**
- **Is missing the required exit trap** — the script must contain `trap 'echo $? > .orchestrator.exit' EXIT`
- **Matches any dangerous pattern**, including:

| Category | Blocked patterns |
|----------|-----------------|
| Destructive commands | `rm` targeting `/` or `~`, `mkfs`, `dd` to/from devices |
| Sensitive file access | `/etc/passwd`, `/etc/shadow`, `/etc/sudoers`, `/proc/`, `/sys/`, `~/.ssh/`, `/root/`, Docker socket |
| Network tools | `curl`, `wget`, `nc`, `ncat`, `socat`, `ssh`, `scp`, `sftp`, `telnet`, `rsync` |
| Reverse shells | `/dev/tcp/`, `/dev/udp/` |
| Privilege escalation | `sudo`, `su`, `chown`, dangerous `chmod` |
| Container/system escape | `chroot`, `nsenter`, `unshare`, `mount`, `umount`, `docker`, `kubectl` |
| Kernel manipulation | `sysctl`, `modprobe`, `insmod`, `rmmod`, `iptables`, `nftables` |
| Obfuscated execution | `base64 \| bash`, `eval`, `python -c`, `perl -e`, `ruby -e` |
| Persistence | `crontab`, `/etc/cron`, `systemctl`, `service`, `at` |
| Fork bombs | `:(){ ...\|: }` |
| Resource exhaustion | `stress`, `stress-ng` |
| Crypto miners | `xmrig`, `minerd`, `cpuminer` |
| Environment secrets | `$AWS_*`, `$SECRET`, `$TOKEN`, `$PASSWORD`, `$API_KEY` |

**This is a sanity check, not a sandbox.** It can be bypassed by
determined actors. Input scripts are still expected to come from trusted
or semi-trusted sources. True isolation must be enforced at the
deployment level using the container hardening measures below.

### Path Traversal Protection

All ZIP file operations include automatic path traversal protection.
During extraction, paths are canonicalized and checked to ensure they
remain within the job working directory. Attempts to escape using `..`
in file paths (e.g., `../../etc/passwd`) are rejected with an error.
This protection applies to:
- Job submission (incoming files)
- Result retrieval (partial and complete downloads)
- Both server and client operations

This is a defense-in-depth measure. While file names are already sanitized
on upload, path traversal protection provides an additional safety layer
against malformed archives or implementation bugs.

### Container Hardening

The client executes user-submitted scripts with the full privileges of
the process. Apply all of the following to limit blast radius:

| Measure | Docker Compose | Purpose |
|---------|---------------|---------|
| Read-only rootfs | `read_only: true` | Prevent filesystem tampering |
| Drop all capabilities | `cap_drop: [ALL]` | Remove kernel-level privileges |
| No new privileges | `security_opt: [no-new-privileges:true]` | Block `setuid`/`setgid` escalation |
| CPU limit | `deploy.resources.limits.cpus` | Prevent CPU starvation |
| Memory limit | `deploy.resources.limits.memory` | Prevent OOM on host |
| PIDs limit | `deploy.resources.limits.pids` | Prevent fork bombs |
| Internal network | `networks: [internal]` | Block outbound internet access |
| Writable tmpfs | `tmpfs: [/tmp]` | Provide scratch space on read-only rootfs |

Example (applied to the client service):

```yaml
services:
  client:
    read_only: true
    cap_drop:
      - ALL
    security_opt:
      - no-new-privileges:true
    tmpfs:
      - /tmp
    deploy:
      resources:
        limits:
          cpus: "2"
          memory: 2G
          pids: 256
    networks:
      - internal

networks:
  internal:
    internal: true
```

**Note:** The Kubernetes manifests already run as non-root (`runAsUser: 1000`).
For Docker deployments, the Dockerfile does not yet create a dedicated non-root
user — see the TODO in the Dockerfile.

### Network Security

1. **Never expose clients to the internet**
   - Clients execute user-submitted scripts
   - Use internal networks only
   - Block all outbound access from client containers

2. **Use a reverse proxy**
   - TLS termination
   - Rate limiting
   - Request filtering

3. **Firewall rules**
   ```bash
   # Allow only orchestrator server to reach clients
   iptables -A INPUT -p tcp --dport 9000 -s <server-ip> -j ACCEPT
   iptables -A INPUT -p tcp --dport 9000 -j DROP
   ```

### Reverse Proxy (nginx)

```nginx
# http block — limit_req_zone must be at http context, not inside server
http {
    limit_req_zone $binary_remote_addr zone=upload:10m rate=10r/s;

    upstream orchestrator {
        server 127.0.0.1:5000;
    }

    server {
        listen 443 ssl http2;
        server_name jobs.example.com;

        ssl_certificate /etc/nginx/certs/cert.pem;
        ssl_certificate_key /etc/nginx/certs/key.pem;

        location /upload {
            limit_req zone=upload burst=20 nodelay;
            client_max_body_size 400M;
            proxy_pass http://orchestrator;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
        }

        location /download {
            proxy_pass http://orchestrator;
            proxy_set_header Host $host;
        }

        location /terminate {
            proxy_pass http://orchestrator;
            proxy_set_header Host $host;
        }

        location /health {
            proxy_pass http://orchestrator;
        }

        # Block swagger in production (optional)
        location /swagger {
            deny all;
        }
    }
}
```

### Authentication

job-orchestrator does not implement authentication. Options:

1. **Reverse proxy authentication**
   ```nginx
   location / {
       auth_basic "Restricted";
       auth_basic_user_file /etc/nginx/.htpasswd;
       proxy_pass http://orchestrator;
   }
   ```

2. **Application-level authentication**
   - Wrap the API in your application
   - Validate users before calling job-orchestrator

3. **OAuth2 Proxy**
   - Use oauth2-proxy in front of the service
   - Integrates with identity providers

## Resource Planning

### Server Requirements

| Load Level | CPU | Memory | Storage |
|------------|-----|--------|---------|
| Light (< 100 jobs/day) | 1 core | 512MB | 10GB |
| Medium (100-1000 jobs/day) | 2 cores | 1GB | 50GB |
| Heavy (> 1000 jobs/day) | 4 cores | 2GB | 100GB+ |

Storage depends heavily on job file sizes and retention period.

### Client Requirements

Depends entirely on your job workloads:

| Job Type | CPU | Memory |
|----------|-----|--------|
| Text processing | 1 core | 512MB |
| Scientific computing | 4-8 cores | 8-16GB |
| ML/Deep learning | 8+ cores + GPU | 32GB+ |

### Storage Calculation

```
Storage = (avg_job_size) × (jobs_per_day) × (retention_days)

Example:
- 10MB average job
- 500 jobs/day
- 10 day retention (default MAX_AGE)
= 10MB × 500 × 10 = 50GB
```

## Monitoring

### Health Checks

```bash
# Server health
curl -f http://localhost:5000/health

# Client health
curl -f http://localhost:9000/health

# Client load
curl http://localhost:9000/load
```

### Log Aggregation

```yaml
services:
  server:
    logging:
      driver: "fluentd"
      options:
        fluentd-address: "localhost:24224"
        tag: "job-orchestrator.server"
```

## Backup & Recovery

### What to Backup

1. **Server database** (`DB_PATH`)
   - Contains job history and status
   - Critical for job tracking

2. **Server data directory** (`DATA_PATH`)
   - Contains job files and results
   - Large, may use incremental backups

### Backup Script

```bash
#!/bin/bash
BACKUP_DIR=/backups/job-orchestrator
DATE=$(date +%Y%m%d_%H%M%S)

# Backup database
sqlite3 /opt/data/db.sqlite ".backup '${BACKUP_DIR}/db_${DATE}.sqlite'"

# Backup data (incremental with rsync)
rsync -av --delete /opt/data/ ${BACKUP_DIR}/data/

# Cleanup old backups (keep 7 days)
find ${BACKUP_DIR} -name "db_*.sqlite" -mtime +7 -delete
```

### Recovery

```bash
# Stop server
docker compose stop server

# Restore database
cp /backups/job-orchestrator/db_latest.sqlite /opt/data/db.sqlite

# Restore data
rsync -av /backups/job-orchestrator/data/ /opt/data/

# Start server
docker compose start server
```

## High Availability

### Current Limitations

- Single server architecture
- No built-in clustering
- SQLite doesn't support concurrent writes

### Workarounds

1. **Quick recovery**
   - Automated health checks
   - Container auto-restart
   - Fast backup restoration

2. **Stateless clients**
   - Clients can be restarted freely
   - Jobs are tracked by server

3. **Future improvements**
   - PostgreSQL support (planned)
   - Server clustering (planned)

## Maintenance

### Database Maintenance

```bash
# Vacuum database (reclaim space)
sqlite3 /opt/data/db.sqlite "VACUUM;"

# Check integrity
sqlite3 /opt/data/db.sqlite "PRAGMA integrity_check;"
```

### Log Rotation

Ensure logs don't fill disk:

```yaml
services:
  server:
    logging:
      driver: "json-file"
      options:
        max-size: "50m"
        max-file: "5"
```

### Updates

```bash
# Pull latest image
docker pull ghcr.io/rvhonorato/job-orchestrator:latest

# Recreate containers
docker compose up -d
```

## Troubleshooting

See [Troubleshooting Guide](../troubleshooting.md) for common issues.

## See Also

- [Docker Deployment](./docker.md)
- [Server Configuration](../configuration/server.md)
- [Troubleshooting](../troubleshooting.md)
