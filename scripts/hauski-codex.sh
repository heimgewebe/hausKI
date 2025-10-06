#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-$(pwd)}"
PROMPT_FILE="${2:-scripts/codex-prompts/bugfix.md}"
POLICY="${3:-scripts/policies/codex.policy.yml}"
LOCAL_POLICY="scripts/policies/codex.policy.local.yml"
CODEX_BIN="${CODEX_BIN:-codex}"
CODEX_NPX_SPEC="${CODEX_NPX_SPEC:-@openai/codex@1.0.0}"

cd "$ROOT"
git rev-parse --show-toplevel >/dev/null
test -r "$PROMPT_FILE" && test -r "$POLICY"
[ -r "$LOCAL_POLICY" ] && POLICY_COMBINED="$(mktemp)" || POLICY_COMBINED="$POLICY"

if [ "$POLICY_COMBINED" != "$POLICY" ]; then
  cat "$POLICY" "$LOCAL_POLICY" > "$POLICY_COMBINED"
fi

TS="$(date +%Y-%m-%d_%H-%M-%S)"
REPO="$(basename "$(git rev-parse --show-toplevel)")"
LOGDIR="$HOME/.hauski/review/$REPO/$TS"
mkdir -p "$LOGDIR" .hauski-tmp

git update-index -q --refresh
test -z "$(git status --porcelain)" || { echo "❌ Working tree nicht clean"; exit 1; }

cp "$PROMPT_FILE" "$LOGDIR/prompt.md"
cp "$POLICY_COMBINED" "$LOGDIR/policy.yml"

run_codex() {
  if command -v "$CODEX_BIN" >/dev/null 2>&1; then
    "$CODEX_BIN"
  elif command -v codex >/dev/null 2>&1; then
    codex
  else
    npx -y "$CODEX_NPX_SPEC"
  fi
}

{
  echo "### PROMPT"; cat "$PROMPT_FILE"; echo
  echo "### POLICY"; cat "$POLICY_COMBINED"
} | run_codex | tee "$LOGDIR/session.raw.md"

awk '/^diff --git /{flag=1} flag{print}' "$LOGDIR/session.raw.md" > .hauski-tmp/patch.diff || true
LINES=$(wc -l < .hauski-tmp/patch.diff || echo 0)
[ "$LINES" -ge 5 ] || { echo "❌ Kein Patch."; exit 2; }

FILES=$(grep -c '^diff --git ' .hauski-tmp/patch.diff || true)
if [ "$FILES" -gt 20 ]; then
  echo "❌ Zu viele Dateien im Patch ($FILES > 20)."
  exit 2
fi

CHANGES=$(grep -cE '^[-+][^-+]' .hauski-tmp/patch.diff || true)
if [ "$CHANGES" -gt 500 ]; then
  echo "❌ Patch zu groß ($CHANGES > 500 Änderungen)."
  exit 2
fi

awk '/^diff --git /{exit} {print}' "$LOGDIR/session.raw.md" > "$LOGDIR/plan.md" || true

git apply --3way --check .hauski-tmp/patch.diff || { echo "❌ Patch check failed"; exit 2; }
git apply --3way .hauski-tmp/patch.diff

just test-quick || { echo "❌ Tests rot – rollback"; git restore -SW .; exit 1; }

BR="codex/$TS"
git checkout -b "$BR" 2>/dev/null || git checkout "$BR"
git add -A
cat > .hauski-tmp/commitmsg.txt <<'EOF2'
fix: minimaler Patch via Codex

Warum:
- siehe $TS (HausKI Review Log)

Tests:
- quick suite grün

Co-authored-by: codex <>
EOF2
git commit -F .hauski-tmp/commitmsg.txt

CANVAS="docs/canvas/codex/${TS}.canvas"
mkdir -p "$(dirname "$CANVAS")"
cat > "$CANVAS" <<'JSON'
{
  "nodes":[
    {"id":"meta","type":"text","text":"Codex-Run: Minimaler Patch","x":0,"y":0,"color":"blue"},
    {"id":"prozess","type":"text","text":"Plan → Patch → Test","x":300,"y":0,"color":"yellow"},
    {"id":"risiko","type":"text","text":"Begrenzte Diffgröße, kein Reformat","x":0,"y":150,"color":"red"},
    {"id":"ziel","type":"text","text":"Tests grün; PR vorbereitbar","x":300,"y":150,"color":"green"},
    {"id":"legende","type":"text","text":"Blau=Meta, Grau=Grundlagen, Gelb=Prozess, Rot=Risiken, Grün=Ziele, Violett=Ebenen","x":-200,"y":260,"color":"gray"}
  ],
  "edges":[
    {"fromNode":"meta","toNode":"prozess","label":"Ablauf"},
    {"fromNode":"prozess","toNode":"ziel","label":"Ergebnis"},
    {"fromNode":"prozess","toNode":"risiko","label":"Beachtung"}
  ]
}
JSON

if [ "$POLICY_COMBINED" != "$POLICY" ]; then
  rm -f "$POLICY_COMBINED"
fi

rm -f .hauski-tmp/patch.diff

echo "✅ Committed: $BR"
echo "📒 Logs: $LOGDIR"
echo "🗺️  Canvas: $CANVAS"
