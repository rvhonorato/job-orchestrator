#!/bin/bash
#=============================================================================#
# start_example.sh - Start job-orchestrator server and client for testing
#
# This script starts both the server and client components with
# minimal configuration for local development and testing.
#
# Usage:
#   ./start_example.sh        # Start both server and client
#   ./start_example.sh server  # Start only the server
#   ./start_example.sh client  # Start only the client
#   ./start_example.sh stop    # Stop all running instances
#=============================================================================#

# Use the built binary from target/release, or fall back to system binary
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY="${PROJECT_DIR}/target/release/job-orchestrator"

# Fall back to system binary if local one doesn't exist
if [ ! -x "$BINARY" ]; then
    BINARY="job-orchestrator"
fi
SERVER_PORT=5000
CLIENT_PORT=9000
SERVICE_NAME="example"

start_server() {
  echo "Starting job-orchestrator server on port $SERVER_PORT..."

  # Set required environment variables for the example service
  export SERVICE_EXAMPLE_UPLOAD_URL=http://localhost:$CLIENT_PORT/submit
  export SERVICE_EXAMPLE_DOWNLOAD_URL=http://localhost:$CLIENT_PORT/retrieve
  export SERVICE_EXAMPLE_TERMINATE_URL=http://localhost:$CLIENT_PORT/kill
  export SERVICE_EXAMPLE_RUNS_PER_USER=5

  # Create temporary data directory for the example
  EXAMPLE_DATA_DIR="${PROJECT_DIR}/example_data"
  mkdir -p "$EXAMPLE_DATA_DIR" 2>/dev/null || true

  # Set data paths
  export DB_PATH="${EXAMPLE_DATA_DIR}/server.db"
  export DATA_PATH="${EXAMPLE_DATA_DIR}/server_jobs"

  # Start server in background (PORT is already exported from the service config)
  PORT=$SERVER_PORT $BINARY server &
  SERVER_PID=$!

  echo "Server started with PID: $SERVER_PID"
  echo "Server API available at: http://localhost:$SERVER_PORT/"
  echo "Swagger UI available at: http://localhost:$SERVER_PORT/swagger-ui/"

  # Wait for server to be ready
  sleep 2

  # Verify server is running
  if curl -s http://localhost:$SERVER_PORT/health >/dev/null 2>&1; then
    echo "✓ Server is running and healthy"
  else
    echo "✗ Server failed to start"
    kill $SERVER_PID 2>/dev/null
    exit 1
  fi
}

start_client() {
  echo "Starting job-orchestrator client on port $CLIENT_PORT..."

  # Create temporary data directory for the client
  CLIENT_DATA_DIR="${PROJECT_DIR}/example_data/client"
  mkdir -p "$CLIENT_DATA_DIR" 2>/dev/null || true

  # Start client in background
  PORT=$CLIENT_PORT DB_PATH="${CLIENT_DATA_DIR}/client.db" DATA_PATH="$CLIENT_DATA_DIR" $BINARY client &
  CLIENT_PID=$!

  echo "Client started with PID: $CLIENT_PID"

  # Wait for client to be ready
  sleep 2

  # Verify client is running
  if curl -s http://localhost:$CLIENT_PORT/health >/dev/null 2>&1; then
    echo "✓ Client is running and healthy"
  else
    echo "✗ Client failed to start"
    kill $CLIENT_PID 2>/dev/null
    exit 1
  fi
}

stop_all() {
  echo "Stopping all job-orchestrator instances..."

  # Find and kill all job-orchestrator processes
  pids=$(pgrep -f "$BINARY" 2>/dev/null || true)

  if [ -z "$pids" ]; then
    echo "No running job-orchestrator processes found"
    return 0
  fi

  for pid in $pids; do
    echo "Stopping PID: $pid"
    kill $pid 2>/dev/null || true
  done

  # Wait for processes to terminate
  sleep 1

  # Force kill if still running
  pids=$(pgrep -f "$BINARY" 2>/dev/null || true)
  if [ -n "$pids" ]; then
    echo "Force killing remaining processes..."
    for pid in $pids; do
      kill -9 $pid 2>/dev/null || true
    done
  fi

  echo "✓ All job-orchestrator processes stopped"
}

# Main logic
case "$1" in
server)
  start_server
  ;;
client)
  start_client
  ;;
stop)
  stop_all
  ;;
*)
  echo "Starting job-orchestrator server and client..."
  echo "=============================================="

  start_server
  echo ""
  start_client

  echo ""
  echo "=============================================="
  echo "✓ job-orchestrator is ready!"
  echo ""
  echo "Endpoints:"
  echo "  Server: http://localhost:$SERVER_PORT/"
  echo "  Client: http://localhost:$CLIENT_PORT/"
  echo ""
  echo "Example commands:"
  echo "  # Submit a job"
  echo "  curl -X POST http://localhost:$SERVER_PORT/upload \\"
  echo "    -F 'file=@run.sh' \\"
  echo "    -F 'user_id=1' \\"
  echo "    -F 'service=example'"
  echo ""
  echo "  # Check job status"
  echo "  curl http://localhost:$SERVER_PORT/download/1"
  echo ""
  echo "  # Cancel a job"
  echo "  curl -X POST http://localhost:$SERVER_PORT/terminate/1"
  echo ""
  echo "Press Ctrl+C to stop both server and client"

  # Keep running until interrupted
  while true; do
    sleep 1
  done
  ;;
esac
