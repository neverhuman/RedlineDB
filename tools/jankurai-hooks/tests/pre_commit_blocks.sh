#!/usr/bin/env bash
# Integration test for the staged-files pre-commit hook.
#
# Verifies:
#   1. Hook bootstrap sets core.hooksPath to the tracked hook directory.
#   2. Clean staged file => commit succeeds (exit 0).
#   3. Dirty working-tree content is ignored when the staged blob is clean.
#   4. Staged file with a hard finding => commit blocks with file + report path.
#   5. Same as (4) with JANKURAI_SKIP_HOOKS=1 => commit succeeds.
#   6. Empty staged set (--allow-empty) => commit succeeds.
#
# Usage:
#   bash tools/jankurai-hooks/tests/pre_commit_blocks.sh
#
# Requires `jankurai` 1.5.0 on PATH (or JANKURAI_BIN env var set).
set -euo pipefail

HOOK_DIR="${HOOK_DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
if [ ! -x "$HOOK_DIR/pre-commit" ] || [ ! -x "$HOOK_DIR/prepare-commit-msg" ] || [ ! -x "$HOOK_DIR/install.sh" ]; then
  echo "FAIL: expected executable hooks and installer under $HOOK_DIR" >&2
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

mkdir -p tools
cp -R "$HOOK_DIR" tools/jankurai-hooks

pass_count=0
fail_count=0

note() { printf '  [test] %s\n' "$*"; }
ok()   { pass_count=$((pass_count + 1)); printf '  PASS: %s\n' "$*"; }
bad()  { fail_count=$((fail_count + 1)); printf '  FAIL: %s\n' "$*" >&2; }

# --- Test 1: bootstrap activates tracked hook path ---
note "bootstrap sets core.hooksPath"
if bash tools/jankurai-hooks/install.sh >/dev/null && [ "$(git config --get core.hooksPath)" = "tools/jankurai-hooks" ]; then
  ok "core.hooksPath points at tracked hooks"
else
  bad "bootstrap did not configure tracked hooks"
fi

# Seed initial commit so HEAD exists.
echo "init" > README.md
git add README.md
git commit -q -m "init" || {
  echo "FAIL: initial commit failed" >&2
  exit 1
}

# --- Test 2: clean file passes ---
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

# --- Test 3: staged content is authoritative ---
note "clean staged blob passes despite dirty worktree"
cat > staged_only.rs <<'EOF'
pub fn ok() -> u8 {
    1
}
EOF
git add staged_only.rs
cat > staged_only.rs <<'EOF'
pub fn boom() {
    unsafe { let p = std::ptr::null::<u8>(); println!("{:?}", *p); }
}
EOF
if git commit -q -m "add staged-only clean file"; then
  ok "working-tree-only finding ignored"
else
  bad "working-tree-only finding blocked staged-clean commit"
fi
git checkout -q -- staged_only.rs

# --- Test 4: undocumented unsafe blocks ---
note "rust unsafe-without-SAFETY blocks"
cat > bad.rs <<'EOF'
pub fn boom() {
    unsafe { let p = std::ptr::null::<u8>(); println!("{:?}", *p); }
}
EOF
git add bad.rs
block_stderr="$work/block.err"
if git commit -q -m "add bad.rs" 2>"$block_stderr"; then
  bad "hard finding allowed through"
else
  if grep -q 'jankurai pre-commit: blocked' "$block_stderr" \
      && grep -q 'bad.rs' "$block_stderr" \
      && grep -q 'target/jankurai/hooks/pre-commit-staged.log' "$block_stderr"; then
    ok "hard finding blocked with file and report path"
  else
    bad "block stderr omitted expected file/report details"
    sed -n '1,80p' "$block_stderr" >&2
  fi
fi

# Drop staged but keep file so test 5 can re-stage.
git reset -q HEAD -- bad.rs

# --- Test 5: bypass with JANKURAI_SKIP_HOOKS=1 ---
note "bypass env allows commit"
git add bad.rs
if JANKURAI_SKIP_HOOKS=1 git commit -q -m "bypass bad.rs"; then
  ok "bypass env honored"
else
  bad "bypass env did not honor"
fi

# --- Test 6: --allow-empty with no staged files passes ---
note "empty commit passes"
if git commit -q --allow-empty -m "empty"; then
  ok "empty commit passed"
else
  bad "empty commit blocked"
fi

printf '\nSummary: %d passed, %d failed\n' "$pass_count" "$fail_count"
[ "$fail_count" -eq 0 ]
