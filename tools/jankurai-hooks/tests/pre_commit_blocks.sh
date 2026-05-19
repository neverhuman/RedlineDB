#!/usr/bin/env bash
# Integration test for the staged-files pre-commit hook.
#
# Verifies:
#   1. Clean staged file on a branch that contains origin/main => commit succeeds (exit 0).
#   2. Staged file with a hard finding => commit blocks (exit non-zero).
#   3. Same as (2) with JANKURAI_SKIP_HOOKS=1 => commit succeeds.
#   4. Empty staged set (--allow-empty) => commit succeeds.
#   5. Branch that is behind origin/main => commit blocks before audit.
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

REQUIRE_MAIN_SRC="${REQUIRE_MAIN_SRC:-$(cd "$(dirname "$0")/../../.." && pwd)/ops/ci/require-main-up-to-date.sh}"
if [ ! -f "$REQUIRE_MAIN_SRC" ]; then
  echo "FAIL: freshness gate script not found at $REQUIRE_MAIN_SRC" >&2
  exit 2
fi

JANKURAI_CMD="${JANKURAI_BIN:-$(command -v jankurai || true)}"
if [ -z "$JANKURAI_CMD" ] || [ ! -x "$JANKURAI_CMD" ]; then
  echo "FAIL: jankurai binary not found on PATH or via JANKURAI_BIN" >&2
  exit 2
fi

work="$(mktemp -d "${TMPDIR:-/tmp}/jankurai-hook-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT

origin="$work/origin.git"
git init --bare -q "$origin"

cd "$work"
git init -b main -q repo
cd repo
git config user.email test@example.com
git config user.name test
git config commit.gpgsign false
git remote add origin "$origin"

mkdir -p .git/hooks
cp "$HOOK_SRC" .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
mkdir -p ops/ci
cp "$REQUIRE_MAIN_SRC" ops/ci/require-main-up-to-date.sh

# Seed initial commit so HEAD exists.
echo "init" > README.md
git add README.md
if JANKURAI_SKIP_HOOKS=1 git commit -q -m "init" > "$work/init.out" 2> "$work/init.err"; then
  :
else
  echo "FAIL: initial commit failed" >&2
  cat "$work/init.err" >&2
  exit 1
fi
git push -q -u origin main
git -C "$origin" symbolic-ref HEAD refs/heads/main
git checkout -q -b feature/clean-branch

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
if git commit -q -m "add hello" > "$work/clean.out" 2> "$work/clean.err"; then
  ok "clean file committed"
else
  bad "clean file blocked unexpectedly"
  cat "$work/clean.err" >&2
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
if JANKURAI_SKIP_HOOKS=1 git commit -q -m "bypass bad.rs" > "$work/bypass.out" 2> "$work/bypass.err"; then
  ok "bypass env honored"
else
  bad "bypass env did not honor"
  cat "$work/bypass.err" >&2
fi

# --- Test 4: --allow-empty with no staged files passes ---
note "empty commit passes"
if git commit -q --allow-empty -m "empty" > "$work/empty.out" 2> "$work/empty.err"; then
  ok "empty commit passed"
else
  bad "empty commit blocked"
  cat "$work/empty.err" >&2
fi

# --- Test 5: stale main blocks even for a clean file ---
note "stale branch is blocked before audit"
updater="$work/updater"
if git clone -q --branch main "$origin" "$updater" > "$work/updater.out" 2> "$work/updater.err"; then
  if ( cd "$updater" && \
    git config user.email test@example.com && \
    git config user.name test && \
    echo "remote update" >> README.md && \
    git add README.md && \
    git commit -q -m "advance main" > "$work/updater-commit.out" 2> "$work/updater-commit.err" && \
    git push -q origin main > "$work/updater-push.out" 2> "$work/updater-push.err"
  ); then
    :
  else
    echo "FAIL: updater branch could not advance origin/main" >&2
    cat "$work/updater.err" >&2
    cat "$work/updater-commit.err" >&2
    cat "$work/updater-push.err" >&2
    exit 1
  fi
else
  echo "FAIL: updater clone could not be created" >&2
  cat "$work/updater.err" >&2
  exit 1
fi

git checkout -q feature/clean-branch
cat > stale.md <<'EOF'
# Freshness
This file is clean; only the branch freshness gate should block it.
EOF
git add stale.md
if git commit -q -m "stale branch commit" 2> "$work/stale.err"; then
  bad "stale branch commit was allowed"
else
  if grep -q "does not contain the latest origin/main" "$work/stale.err"; then
    ok "stale branch blocked by freshness gate"
  else
    bad "stale branch failed for the wrong reason"
    cat "$work/stale.err" >&2
  fi
fi

printf '\nSummary: %d passed, %d failed\n' "$pass_count" "$fail_count"
[ "$fail_count" -eq 0 ]
