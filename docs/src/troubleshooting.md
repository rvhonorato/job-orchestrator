# Troubleshooting

Common issues and solutions for job-orchestrator.

## Server Issues

### Server Won't Start

**Symptom**: Server fails to start, exits immediately

**Possible Causes**:

1. **Port already in use**

   ```
   Error: Address already in use
   ```

   Solution:

   ```bash
   # Find process using the port
   lsof -i :5000
   # Kill it or use a different port
   PORT=5001 job-orchestrator server
   ```

2. **Database path not writable**

   ```
   Error: unable to open database file
   ```

   Solution:

   ```bash
   # Check directory exists and is writable
   mkdir -p /opt/data
   chmod 755 /opt/data
   ```

3. **Missing service configuration**

   ```
   Error: No services configured
   ```

   Solution: Configure at least one service:

   ```bash
   export SERVICE_EXAMPLE_UPLOAD_URL=http://client:9000/submit
   export SERVICE_EXAMPLE_DOWNLOAD_URL=http://client:9000/retrieve
   ```

### Jobs Stuck in Queued

**Symptom**: Jobs stay in `Queued` status indefinitely

**Possible Causes**:

1. **Quota exhausted**

   Check if user has reached their limit:
   - Default quota is 5 concurrent jobs per user per service
   - Wait for running jobs to complete, or increase quota

2. **Client unreachable**

   Verify client connectivity:

   ```bash
   curl http://client:9000/health
   ```

3. **Service misconfigured**

   Verify service URLs are correct:

   ```bash
   echo $SERVICE_EXAMPLE_UPLOAD_URL
   curl -X POST $SERVICE_EXAMPLE_UPLOAD_URL  # Should return error, not timeout
   ```

### Jobs Stuck in Submitted

**Symptom**: Jobs move to `Submitted` but never complete

**Possible Causes**:

1. **Client not executing jobs**

   Check client logs for errors

   ```bash
   docker logs client
   ```

2. **`run.sh` hanging**

   Your script may be waiting for input or stuck in a loop

3. **Getter task not running**

   Server may need restart

### Upload Fails with 400

**Symptom**: `POST /upload` returns 400 Bad Request

**Possible Causes**:

1. **Missing required fields**

   ```bash
   # Ensure all fields are provided
   curl -X POST http://localhost:5000/upload \
     -F "file=@run.sh" \
     -F "user_id=1" \      # Required
     -F "service=example"   # Required
   ```

2. **Unknown service**

   Service must be configured on server:

   ```bash
   export SERVICE_EXAMPLE_UPLOAD_URL=...
   ```

3. **File too large**

   Default limit is 400MB. Check file sizes.

## Client Issues

### Client Not Receiving Jobs

**Symptom**: Client running but no jobs arrive

**Check**:

1. **Network connectivity**

   ```bash
   # From server, can you reach client?
   curl http://client:9000/health
   ```

2. **Firewall rules**

   ```bash
   # Client port must be accessible from server
   iptables -L -n | grep 9000
   ```

3. **Docker networking**

   ```bash
   # Containers must be on same network
   docker network inspect job-orchestrator_default
   ```

### Jobs Stuck in Prepared

**Symptom**: Payloads stay in `Prepared` status

**Possible Causes**:

1. **Runner task not running**

   Check client logs, may need restart

2. **`run.sh` not found or not executable**

   Ensure the script exists and is executable:

   ```bash
   # In your upload
   chmod +x run.sh
   ```

3. **Permission issues**

   Client working directory may have permission issues

### Execution Fails

**Symptom**: Jobs complete but with `Failed` status

**Check**:

1. **Exit code**

   `run.sh` must exit with code 0 for success:

   ```bash
   #!/bin/bash
   # Your commands here
   exit 0  # Explicit success
   ```

2. **Script errors**

   Check output files for error messages

3. **Missing dependencies**

   Your script may need tools not available in the container

## Database Issues

### Database Locked

**Symptom**: "database is locked" errors

**Causes**: Multiple processes accessing SQLite

**Solution**:

- Ensure only one server instance runs
- Check for zombie processes
- Restart server

### Database Corrupted

**Symptom**: Strange errors, missing data

**Solution**:

1. Stop server
2. Backup current database
3. Run integrity check:

   ```bash
   sqlite3 db.sqlite "PRAGMA integrity_check;"
   ```

4. If corrupted, restore from backup or delete and restart

### Out of Disk Space

**Symptom**: "disk full" errors

**Solution**:

1. Check disk usage:

   ```bash
   df -h
   ```

2. Clean old jobs:

   ```bash
   # Reduce MAX_AGE and restart
   export MAX_AGE=3600  # 1 hour
   ```

3. Manually clean data directory

## Docker Issues

### Container Exits Immediately

**Check logs**:

```bash
docker logs container_name
```

Common causes:

- Missing environment variables
- Port conflicts
- Permission issues

### Cannot Connect Between Containers

**Ensure same network**:

```yaml
services:
  server:
    networks:
      - app-network
  client:
    networks:
      - app-network

networks:
  app-network:
```

**Use service names, not localhost**:

```bash
# Wrong
SERVICE_EXAMPLE_UPLOAD_URL=http://localhost:9000/submit

# Correct
SERVICE_EXAMPLE_UPLOAD_URL=http://client:9000/submit
```

### Volume Permission Issues

**Symptom**: Permission denied when writing to volumes

**Solution**:

```yaml
services:
  server:
    user: "1000:1000"  # Match host user
    volumes:
      - ./data:/opt/data
```

Or fix permissions:

```bash
sudo chown -R 1000:1000 ./data
```

## Performance Issues

### Slow Job Processing

**Possible Causes**:

1. **Slow database**
   - Use SSD storage for database
   - Run VACUUM periodically

2. **Network latency**
   - Place server and clients on same network
   - Check for packet loss

3. **Client overloaded**
   - Add more clients
   - Reduce RUNS_PER_USER

### High Memory Usage

**Server**:

- Memory grows with job count
- Clean old jobs with lower MAX_AGE

**Client**:

- In-memory database grows with payloads
- Restart client to clear

### Disk Usage Growing

**Check**:

```bash
du -sh /opt/data/*
```

**Solutions**:

- Reduce MAX_AGE
- Increase cleanup frequency
- Archive old results externally

## Getting Help

If you can't resolve an issue:

1. **Check logs** for specific error messages
2. **Search existing issues**: [GitHub Issues](https://github.com/rvhonorato/job-orchestrator/issues)
3. **Open new issue** with:
   - Version
   - Configuration
   - Steps to reproduce
   - Logs

## See Also

- [Server Configuration](./configuration/server.md)
- [Client Configuration](./configuration/client.md)
- [Docker Deployment](./deployment/docker.md)
