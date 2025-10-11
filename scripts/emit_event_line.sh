#!/usr/bin/env bash
set -euo pipefail
# schreibt eine valide Event-Zeile (JSONL) nach ~/.hauski/events/YYYY-MM.jsonl
ID=${1:-$(date +%s%N)}
NODE_ID=${NODE_ID:-$(hostname)}
KIND=${KIND:-"debug.test"}
TS=$(date +%s%3N)
PAYLOAD=${PAYLOAD:-'{"ok":true}'}

outdir="${HAUSKI_DATA:-$HOME/.hauski}/events"
mkdir -p "$outdir"
outfile="$outdir/$(date +%Y-%m).jsonl"
jq -n --arg id "$ID" --arg node "$NODE_ID" --arg kind "$KIND" --argjson ts "$TS" --argjson pl "$PAYLOAD" '
  {id:$id,node_id:$node,ts:$ts,kind:$kind,payload: ($pl|type=="string" ? ($pl|fromjson) : $pl)}
' >> "$outfile"
echo "wrote $outfile"
