#!/usr/bin/env bash
# Integration test for the staged-files pre-commit hook.
#
# Verifies:
#   1. Clean staged file => commit succeeds (exit 0).
#   2. Staged file with a hard finding => commit blocks (exit non-zero).
#   3. Same as (2) with JANKURAI_SKIP_HOOKS=1 => commit succeeds.
#   4. Empty staged set (--allow-empty) => commit succeeds.
#
# Usage:
#   bash tools/jankurai-hooks/tests/pre_commit_blocks.sh
#
# Requires `jankurai` 1.4.3+ on PATH (or JANKURAI_BIN env var set).
set -euo pipefail

HOOK_SRC="${HOOK_SRC:-$(cd "$(dirname "$0")/.." && pwd)/pre-commit}"
if [ ! -x "$HOOK_SRC" ]; then
  echo "FAIL: hook not executable at $HOOK_SRC" >&2
  exit 2
fi

JANKURAI_CMD="${JANKURAI_BIN:-$(command -v jankurai || true)}"
if [ -z "$JANKURAI_CMD" ] || [ ! -x "$JANKURAI_CMD" ]; then
  echo "FAIL: jankurai binary not found on PATH or via JANKURAI_BIN" >&2
  exit 2
fi

work="$(mktemp -d "${TMPDIR:-/tmp}/jankurai-hook-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT

cd "$work"
git init -q
git config user.email test@example.com
git config user.name test
git config commit.gpgsign false

mkdir -p .git/hooks
cp "$HOOK_SRC" .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit

# Seed initial commit so HEAD exists.
echo "init" > README.md
git add README.md
git commit -q -m "init" || {
  echo "FAIL: initial commit failed" >&2
  exit 1
}

pass_count=0
fail_count=0

note() { printf '  [test] %s\n' "$*"; }
ok()   { pass_count=$((pass_count + 1)); printf '  PASS: %s\n' "$*"; }
bad()  { fail_count=$((fail_count + 1)); printf '  FAIL: %s\n' "$*" >&2; }

# --- Test 1: clean file passes ---
note "clean file passes"
cat > hello.md <<'EOF'
# Hello
This file is clean.
EOF
git add hello.md
if git commit -q -m "add hello"; then
  ok "clean file committed"
else
  bad "clean file blocked unexpectedly"
fi

# --- Test 2: undocumented unsafe blocks ---
note "rust unsafe-without-SAFETY blocks"
cat > bad.rs <<'EOF'
pub fn boom() {
    unsafe { let p = std::ptr::null::<u8>(); println!("{:?}", *p); }
}
EOF
git add bad.rs
if git commit -q -m "add bad.rs" 2>/dev/null; then
  bad "hard finding allowed through"
else
  ok "hard finding blocked"
fi

# Drop staged but keep file so test 3 can re-stage.
git reset -q HEAD -- bad.rs

# --- Test 3: bypass with JANKURAI_SKIP_HOOKS=1 ---
note "bypass env allows commit"
git add bad.rs
if JANKURAI_SKIP_HOOKS=1 git commit -q -m "bypass bad.rs"; then
  ok "bypass env honored"
else
  bad "bypass env did not honor"
fi

# --- Test 4: --allow-empty with no staged files passes ---
note "empty commit passes"
if git commit -q --allow-empty -m "empty"; then
  ok "empty commit passed"
else
  bad "empty commit blocked"
fi

printf '\nSummary: %d passed, %d failed\n' "$pass_count" "$fail_count"
[ "$fail_count" -eq 0 ]
