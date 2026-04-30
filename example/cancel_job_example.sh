#!/bin/bash
#=============================================================================#
# Example: Demonstrating job cancellation with job-orchestrator
#
# This script shows how to:
# 1. Submit a long-running job
# 2. Check its status
# 3. Cancel it before completion
# 4. Verify it was killed
#=============================================================================#

set -e

# Configuration - adjust these to match your setup
SERVER_URL="http://localhost:5000"
SERVICE_NAME="example"
USER_ID=1

# Step 1: Create a long-running job script
echo "Creating long-running job..."
cat >/tmp/run.sh <<'EOF'
#!/bin/bash
# Required: Capture exit code for the orchestrator client to detect job completion/failure
trap 'echo "$?" > .orchestrator.exit' EXIT

# A script that runs for 30 seconds
echo "Starting long-running job at $(date)" > job_output.txt
for i in {1..30}; do
    echo "Progress: $i/30" >> job_output.txt
    sleep 1
done
echo "Job completed at $(date)" >> job_output.txt
EOF
chmod +x /tmp/run.sh

# Step 2: Submit the job
echo "Submitting job..."
JOB_RESPONSE=$(curl -s -X POST "$SERVER_URL/upload" \
  -F "file=@/tmp/run.sh" \
  -F "user_id=$USER_ID" \
  -F "service=$SERVICE_NAME")

JOB_ID=$(echo "$JOB_RESPONSE" | jq -r '.id')
echo "Job submitted with ID: $JOB_ID"

# Step 3: Wait a few seconds for the job to start
echo "Waiting for job to start..."
sleep 5

# Step 4: Check job status
echo "Checking job status..."
STATUS_RESPONSE=$(curl -s "$SERVER_URL/download/$JOB_ID")
STATUS=$(echo "$STATUS_RESPONSE" | jq -r '.status')
echo "Current status: $STATUS"

# Step 5: Cancel the job
echo "Canceling job..."
CANCEL_RESPONSE=$(curl -s -X POST "$SERVER_URL/terminate/$JOB_ID")
CANCEL_STATUS=$(echo "$CANCEL_RESPONSE" | jq -r '.status')
CANCEL_MESSAGE=$(echo "$CANCEL_RESPONSE" | jq -r '.message')
echo "Cancel response - Status: $CANCEL_STATUS, Message: $CANCEL_MESSAGE"

# Step 6: Verify job was killed
echo "Verifying job was killed..."
sleep 2
FINAL_RESPONSE=$(curl -s "$SERVER_URL/download/$JOB_ID")
FINAL_STATUS=$(echo "$FINAL_RESPONSE" | jq -r '.status')
echo "Final status: $FINAL_STATUS"

if [ "$FINAL_STATUS" = "Killed" ]; then
  echo "✓ SUCCESS: Job was successfully canceled!"
else
  echo "✗ FAILURE: Job status is '$FINAL_STATUS', expected 'Killed'"
  exit 1
fi

# Cleanup
rm /tmp/run.sh

echo ""
echo "Example completed successfully!"
