#!/bin/bash

# Example: Using partial download for debugging stuck jobs
# This script demonstrates the /download_partial endpoint

BASE_URL="http://localhost:5000"

echo "=== Job Orchestrator - Partial Download Example ==="
echo ""

# Create a test run script that takes time
cat > run.sh << 'EOF'
#!/bin/bash
trap 'echo "$?" > .orchestrator.exit' EXIT

echo "Starting long-running job..."
for i in $(seq 1 10); do
    echo "Step $i/10" >> progress.txt
    sleep 1
done
echo "Done!" >> progress.txt
EOF
chmod +x run.sh

# Submit a job
echo "1. Submitting job with user_id parameter..."
RESPONSE=$(curl -s -X POST "${BASE_URL}/upload" \
  -F "file=@run.sh" \
  -F "user_id=1" \
  -F "service=example")
echo "$RESPONSE"
echo ""

JOB_ID=$(echo "$RESPONSE" | jq -r '.id')
echo "Job ID: $JOB_ID"
echo ""

# Wait a moment then check partial status
echo "2. Waiting 2 seconds..."
sleep 2
echo ""

# Check partial download
echo "3. Getting partial results (even if job is incomplete)..."
curl -s -o partial_results.zip "${BASE_URL}/download_partial/${JOB_ID}"
echo ""
echo "Partial results saved to partial_results.zip"
echo ""
echo "Contents of partial results:"
unzip -l partial_results.zip 2>/dev/null || echo "No files yet or job completed"
echo ""

# Check final download
echo "4. Waiting for job completion (takes about 10 seconds)..."
sleep 12
echo ""

echo "5. Getting final results..."
curl -s -o final_results.zip "${BASE_URL}/download/${JOB_ID}"
echo ""
echo "Final results saved to final_results.zip"
echo ""
echo "Contents of final results:"
unzip -l final_results.zip
echo ""

echo "=== Example complete ==="
