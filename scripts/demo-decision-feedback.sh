#!/usr/bin/env bash
# Demo script for Decision Feedback API
set -euo pipefail

CORE_URL="${CORE_URL:-http://127.0.0.1:8080}"

echo "=== Decision Feedback API Demo ==="
echo ""

# 1. Insert some test documents
echo "1. Inserting test documents..."
curl -s -X POST "$CORE_URL/index/upsert" \
  -H 'Content-Type: application/json' \
  -d '{
    "doc_id": "rust-guide",
    "namespace": "code",
    "chunks": [{
      "chunk_id": "rust-guide#0",
      "text": "Rust programming language with memory safety guarantees"
    }],
    "meta": {},
    "source_ref": {
      "origin": "chronik",
      "id": "rust-guide-doc",
      "trust_level": "high"
    }
  }' | jq -r '.status'

curl -s -X POST "$CORE_URL/index/upsert" \
  -H 'Content-Type: application/json' \
  -d '{
    "doc_id": "python-guide",
    "namespace": "code",
    "chunks": [{
      "chunk_id": "python-guide#0",
      "text": "Python scripting tutorial for beginners"
    }],
    "meta": {},
    "source_ref": {
      "origin": "external",
      "id": "python-guide-doc",
      "trust_level": "low"
    }
  }' | jq -r '.status'

echo "✓ Documents inserted"
echo ""

# 2. Perform a weighted search (triggers snapshot emission)
echo "2. Performing weighted search..."
SEARCH_RESULT=$(curl -s -X POST "$CORE_URL/index/search" \
  -H 'Content-Type: application/json' \
  -d '{
    "query": "Rust memory safety",
    "k": 5,
    "namespace": "code",
    "include_weights": true
  }')

echo "$SEARCH_RESULT" | jq '{
  matches_count: (.matches | length),
  top_result: .matches[0].doc_id,
  top_score: .matches[0].score,
  weights: .matches[0].weights
}'

echo "✓ Search completed"
echo ""

# 3. List decision snapshots
echo "3. Listing decision snapshots..."
SNAPSHOTS=$(curl -s "$CORE_URL/index/decisions/snapshot")
SNAPSHOT_COUNT=$(echo "$SNAPSHOTS" | jq '.snapshots | length')
echo "Found $SNAPSHOT_COUNT snapshot(s)"

if [ "$SNAPSHOT_COUNT" -gt 0 ]; then
  DECISION_ID=$(echo "$SNAPSHOTS" | jq -r '.snapshots[0].decision_id')
  echo "Decision ID: $DECISION_ID"
  echo ""
  
  # Show snapshot details
  echo "Snapshot details:"
  echo "$SNAPSHOTS" | jq '.snapshots[0] | {
    decision_id,
    intent,
    namespace,
    candidates_count: (.candidates | length),
    selected_id,
    top_candidate: .candidates[0]
  }'
  echo ""
  
  # 4. Record outcome for this decision
  echo "4. Recording outcome (success)..."
  curl -s -X POST "$CORE_URL/index/decisions/outcome" \
    -H 'Content-Type: application/json' \
    -d "{
      \"decision_id\": \"$DECISION_ID\",
      \"outcome\": \"success\",
      \"signal_source\": \"user\",
      \"timestamp\": \"$(date -Iseconds)\",
      \"notes\": \"User confirmed this result was helpful\"
    }" | jq -r '.status'
  
  echo "✓ Outcome recorded"
  echo ""
  
  # 5. Retrieve the outcome
  echo "5. Retrieving outcome..."
  curl -s "$CORE_URL/index/decisions/outcome/$DECISION_ID" | jq '{
    decision_id,
    outcome,
    signal_source,
    notes
  }'
  echo ""
  
  # 6. List all outcomes
  echo "6. Listing all outcomes..."
  curl -s "$CORE_URL/index/decisions/outcomes" | jq '{
    total_outcomes: (.outcomes | length),
    outcomes: [.outcomes[] | {decision_id, outcome, signal_source}]
  }'
  echo ""
fi

echo "=== Demo completed ==="
echo ""
echo "Next steps:"
echo "- heimlern can now fetch snapshots and outcomes"
echo "- Analyze patterns: which weights lead to success?"
echo "- Adjust policies based on feedback"
echo ""
echo "Remember: hausKI does NOT interpret feedback - that's heimlern's job!"
