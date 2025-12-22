#!/usr/bin/env bash
set -euo pipefail

# scripts/generate-test-summary.sh
# Generates a canonical artifact of test truth: artifacts/test.summary.json

ARTIFACT_DIR="artifacts"
ARTIFACT_FILE="${ARTIFACT_DIR}/test.summary.json"
mkdir -p "$ARTIFACT_DIR"

echo "Generating test artifact..."

# Capture metadata
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
COMMIT=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")

# Run tests and capture output
# We use a temporary file to parse results, while also streaming to stdout for the user
TEMP_LOG=$(mktemp)
set +e # Don't exit on test failure immediately, we want to record the failure status
cargo test --workspace -- --nocapture 2>&1 | tee "$TEMP_LOG"
EXIT_CODE=$?
set -e

# Parse results
# Robust parsing using regex to match standard Rust test output:
# "test result: ok. 65 passed; 0 failed; ..."
# "test result: FAILED. 64 passed; 1 failed; ..."
MATCHES=$(grep -E "test result: .*\. [0-9]+ passed; [0-9]+ failed" "$TEMP_LOG" || true)

if [ -z "$MATCHES" ]; then
    echo "❌ Error: No valid test result lines found in output."
    echo "This indicates a parsing failure or a cargo failure preventing tests from running."
    rm "$TEMP_LOG"
    exit 1
fi

# Sum up all "passed" and "failed" counts from all matches
# Extract numbers using sed and sum with awk
TOTAL_PASSED=$(echo "$MATCHES" | sed -E 's/.* ([0-9]+) passed.*/\1/' | awk '{s+=$1} END {print s+0}')
TOTAL_FAILED=$(echo "$MATCHES" | sed -E 's/.* ([0-9]+) failed.*/\1/' | awk '{s+=$1} END {print s+0}')

STATUS="success"
if [ $EXIT_CODE -ne 0 ]; then
    STATUS="failure"
fi

# Sanity check: If status is success but we have failures in parsing (should be covered by exit code, but double check)
if [ "$TOTAL_FAILED" -gt 0 ]; then
    STATUS="failure"
fi

# Generate JSON
cat <<EOF > "$ARTIFACT_FILE"
{
  "meta": {
    "generated_at": "$TIMESTAMP",
    "git_commit": "$COMMIT",
    "git_branch": "$BRANCH"
  },
  "truth": {
    "status": "$STATUS",
    "exit_code": $EXIT_CODE,
    "tests_passed": $TOTAL_PASSED,
    "tests_failed": $TOTAL_FAILED
  },
  "policy": {
    "statement": "If it is not in this artifact, it is not verified truth."
  }
}
EOF

rm "$TEMP_LOG"

echo ""
echo "✅ Truth artifact generated: $ARTIFACT_FILE"
cat "$ARTIFACT_FILE"
echo ""

if [ "$STATUS" = "failure" ]; then
    echo "❌ Tests failed. Artifact records this failure."
    exit 1
fi
