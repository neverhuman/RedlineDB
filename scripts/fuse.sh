#!/usr/bin/env bash
# scripts/fuse.sh — RedlineDB "git repo fusion for development".
#
# Side-by-side checkout of the three sibling repos into a gitignored .fusion/
# tree, plus a generated dev README + helper script. This is NOT a unified Cargo
# workspace: this repo stays thin (no root Cargo.toml, no cross-repo path deps).
# Each sibling builds and is CI'd independently; .fusion/ only wires them
# together for local end-to-end iteration. Nothing here is tracked by this repo —
# every artifact lands under .fusion/, which must be gitignored.
#
# End users never need this — they just `curl install.sh | bash` for the binary.
#
# Env:
#   FUSION_SOURCE  github (default) | jeryu   which clone URLs from family.json
#   FUSION_DIR     workspace dir (default <repo>/.fusion)
#   FUSION_REF_CORE / FUSION_REF_WEB / FUSION_REF_TESTING
#                  pin a sibling to a tag or 40-hex SHA (wins over fusion.lock)
#
# Reproducible pinning: create .fusion/fusion.lock with whitespace-separated
# "<name> <ref>" lines ('#' comments + blanks ignored), e.g.
#   redline-core     v4.1.0
#   redline-web      0123abcd...   (40-hex sha)
#   redline-testing  v1.0.1
# No pin for a repo => it floats (pull --ff-only on its current branch).
set -Eeuo pipefail

log()  { printf '[fuse] %s\n' "$*"; }
warn() { printf '[fuse] WARN: %s\n' "$*" >&2; }
die()  { printf '[fuse] ERROR: %s\n' "$*" >&2; exit 1; }

command -v git     >/dev/null 2>&1 || die "git not found"
command -v python3 >/dev/null 2>&1 || die "python3 not found (used to read family.json)"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
if git -C "$REPO_ROOT" rev-parse --show-toplevel >/dev/null 2>&1; then
  REPO_ROOT="$(git -C "$REPO_ROOT" rev-parse --show-toplevel)"
fi

FUSION_DIR="${FUSION_DIR:-$REPO_ROOT/.fusion}"
FAMILY_JSON="$REPO_ROOT/family.json"
LOCK="$FUSION_DIR/fusion.lock"
SOURCE="${FUSION_SOURCE:-github}"

[ -f "$FAMILY_JSON" ] || die "family.json not found at $FAMILY_JSON"
case "$SOURCE" in
  github|jeryu) ;;
  *) die "FUSION_SOURCE must be 'github' or 'jeryu' (got '$SOURCE')" ;;
esac

# Refuse to run unless .fusion/ is gitignored — sibling trees must never be
# tracked by the thin hub. (Only enforced for the default in-repo workspace.)
# Query with a trailing slash so a dir-only pattern ('/.fusion/') matches even
# before the directory exists.
if [ "$FUSION_DIR" = "$REPO_ROOT/.fusion" ] \
   && ! git -C "$REPO_ROOT" check-ignore -q .fusion/ 2>/dev/null; then
  die ".fusion/ is not gitignored — add '/.fusion/' to .gitignore before fusing."
fi

mkdir -p "$FUSION_DIR"
declare -A RESOLVED=()

# Print the clone URL for repo $1 from family.json given the selected source.
clone_url() {
  python3 - "$FAMILY_JSON" "$1" "$SOURCE" <<'PY'
import json, sys
fp, name, source = sys.argv[1], sys.argv[2], sys.argv[3]
data = json.load(open(fp))
key = "github_clone" if source == "github" else "jeryu_clone"
for r in data["repos"]:
    if r["name"] == name:
        print(r[key]); break
else:
    sys.exit("no repo named %s in family.json" % name)
PY
}

# Echo the pin (tag/sha) for repo $1, or empty when it should float.
pin_for() {
  local name="$1" envvar="" ref="" lname lref _
  case "$name" in
    redline-core)    envvar="FUSION_REF_CORE" ;;
    redline-web)     envvar="FUSION_REF_WEB" ;;
    redline-testing) envvar="FUSION_REF_TESTING" ;;
  esac
  ref="${!envvar:-}"
  if [ -n "$ref" ]; then printf '%s' "$ref"; return; fi
  if [ -f "$LOCK" ]; then
    while read -r lname lref _; do
      [ -z "${lname:-}" ] && continue
      case "$lname" in \#*) continue ;; esac
      if [ "$lname" = "$name" ]; then printf '%s' "$lref"; return; fi
    done < "$LOCK"
  fi
  printf ''
}

sync_repo() {
  local name="$1"
  local dir="$FUSION_DIR/$name" url ref def sha base cand
  url="$(clone_url "$name")"
  ref="$(pin_for "$name")"

  if [ ! -d "$dir/.git" ]; then
    log "clone $name  <-  $url"
    git clone --quiet "$url" "$dir"
  else
    log "fetch $name"
    git -C "$dir" remote set-url origin "$url"   # honor a github<->jeryu switch
    git -C "$dir" fetch --quiet --tags --prune origin
  fi

  # Recover from a remote whose default HEAD is missing/misconfigured (e.g. a
  # mirror whose HEAD points at a branch that doesn't exist): if no commit is
  # checked out, adopt a sensible default branch from the fetched remote refs.
  if ! git -C "$dir" rev-parse --verify -q HEAD >/dev/null 2>&1; then
    base=""
    for cand in main master; do
      if git -C "$dir" show-ref --verify -q "refs/remotes/origin/$cand"; then base="$cand"; break; fi
    done
    if [ -z "$base" ]; then
      base="$(git -C "$dir" for-each-ref --format='%(refname:short)' refs/remotes/origin \
              | grep -v '/HEAD$' | sed 's#^origin/##' | head -n1 || true)"
    fi
    [ -n "$base" ] || die "$name: cloned but no branch to check out (empty remote?)"
    git -C "$dir" checkout -B "$base" "origin/$base" >/dev/null 2>&1 \
      || die "$name: could not check out origin/$base"
    log "$name: adopted default branch '$base' (remote HEAD was unset/misconfigured)"
  fi

  if [ -n "$ref" ]; then
    if ! git -C "$dir" diff --quiet || ! git -C "$dir" diff --cached --quiet; then
      die "$name has local changes; refusing to checkout pin '$ref' (clean $dir first)"
    fi
    git -C "$dir" -c advice.detachedHead=false checkout --quiet --detach "$ref" 2>/dev/null \
      || git -C "$dir" -c advice.detachedHead=false checkout --quiet --detach "origin/$ref" 2>/dev/null \
      || die "cannot resolve pin '$ref' for $name"
    log "$name pinned -> $ref"
  elif git -C "$dir" symbolic-ref -q HEAD >/dev/null; then
    git -C "$dir" pull --quiet --ff-only \
      || warn "$name: ff-only pull failed (diverged or dirty) — left as-is"
  else
    # Floating but detached (e.g. a previous pinned run): rejoin the remote
    # default branch so ff-only updates resume. Never a destructive reset.
    def="$(git -C "$dir" symbolic-ref -q --short refs/remotes/origin/HEAD || true)"  # origin/<branch>
    if [ -n "$def" ]; then
      if git -C "$dir" checkout --quiet "${def#origin/}"; then
        git -C "$dir" pull --quiet --ff-only \
          || warn "$name: ff-only pull failed after restoring '${def#origin/}'"
      else
        warn "$name: could not restore floating branch from $def"
      fi
    else
      warn "$name detached with no pin and no origin/HEAD; leaving at current commit"
    fi
  fi

  sha="$(git -C "$dir" rev-parse HEAD)"
  RESOLVED["$name"]="$sha"
  log "$name @ ${sha:0:12}"
}

for repo in redline-core redline-web redline-testing; do
  sync_repo "$repo"
done

# ---- generated dev helper (gitignored, regenerated every run) ---------------
DEV_SH="$FUSION_DIR/dev.sh"
cat > "$DEV_SH" <<'EOF'
#!/usr/bin/env bash
# GENERATED by scripts/fuse.sh — do not edit, do not commit. Rerun `just fuse`.
set -Eeuo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORE="$HERE/redline-core"; WEB="$HERE/redline-web"; TESTING="$HERE/redline-testing"
ENGINE_BIN="$CORE/target/release/redlinedb"
TESTING_BIN="$TESTING/target/release/redline-testing"
WEB_BIN="$WEB/target/release/redline-web"
SQLITE_BIN="${SQLITE_BIN:-sqlite3}"
DEMO_DB="${DEMO_DB:-$HERE/demo.redline.db}"
WEB_BIND="${WEB_BIND:-127.0.0.1:7788}"

build_core()    { ( cd "$CORE" && cargo build --release -p redlinedb-cli ); }
build_web()     { ( cd "$WEB/apps/web" && (npm ci --no-audit --no-fund || npm install --no-audit --no-fund) && npm run build ); \
                  ( cd "$WEB" && cargo build --release --locked ); }
build_testing() { ( cd "$TESTING" && cargo build --release ); }
build_all()     { build_core; build_web; build_testing; }

seed()          { build_core; "$ENGINE_BIN" "$DEMO_DB" \
                    "CREATE TABLE IF NOT EXISTS t(id INTEGER PRIMARY KEY, name TEXT); INSERT INTO t(name) VALUES('hello');"; }

test_all()      { build_core; build_testing
                  "$TESTING_BIN" run --suite "${SUITE:-sqlite_parity}" \
                    --target-bin "$ENGINE_BIN" --sqlite-bin "$SQLITE_BIN" \
                    --output "$HERE/parity.jsonl" "$@"; }

run_stack()     { build_core; build_web; seed
                  "$WEB_BIN" --target-bin "$ENGINE_BIN" --db "$DEMO_DB" --bind "$WEB_BIND"; }

cmd="${1:-help}"; shift || true
case "$cmd" in
  build-all)     build_all ;;
  build-core)    build_core ;;
  build-web)     build_web ;;
  build-testing) build_testing ;;
  seed)          seed ;;
  test-all)      test_all "$@" ;;
  run-stack)     run_stack ;;
  *) echo "usage: dev.sh {build-all|build-core|build-web|build-testing|seed|test-all|run-stack}" ;;
esac
EOF
chmod +x "$DEV_SH"

# ---- generated dev README (gitignored) --------------------------------------
GEN_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
cat > "$FUSION_DIR/README.dev.md" <<EOF
# RedlineDB — fused dev tree (.fusion/)

Generated by scripts/fuse.sh at ${GEN_AT} (source: ${SOURCE}).
DO NOT COMMIT — this tree is gitignored and is not part of the thin hub.

## Resolved revisions
- redline-core     ${RESOLVED[redline-core]}
- redline-web      ${RESOLVED[redline-web]}
- redline-testing  ${RESOLVED[redline-testing]}

The engine binary is **target/release/redlinedb** (cargo bin \`redlinedb\`,
package \`redlinedb-cli\`). The published end-user command is \`redline\`; the raw
cargo dev artifact is \`redlinedb\`.

## One-shot helpers
- \`./.fusion/dev.sh build-all\`   build all three repos
- \`./.fusion/dev.sh run-stack\`   engine-backed web console on http://127.0.0.1:7788
- \`./.fusion/dev.sh test-all\`    conformance harness vs the built engine (SUITE=... to choose)

## Manual dev loop
\`\`\`bash
# 1) engine
( cd .fusion/redline-core && cargo build --release -p redlinedb-cli )
ENGINE="\$PWD/.fusion/redline-core/target/release/redlinedb"

# 2) seed / smoke
"\$ENGINE" /tmp/demo.redline.db "CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT); INSERT INTO t(name) VALUES('hello'); SELECT * FROM t;"

# 3) harness (redline-testing) against the engine
( cd .fusion/redline-testing && cargo build --release )
.fusion/redline-testing/target/release/redline-testing run \\
  --suite sqlite_parity --target-bin "\$ENGINE" --sqlite-bin /usr/bin/sqlite3 --output /tmp/parity.jsonl

# 4) console (redline-web) pointed at the engine
( cd .fusion/redline-web/apps/web && (npm ci --no-audit --no-fund || npm install --no-audit --no-fund) && npm run build )
( cd .fusion/redline-web && cargo build --release --locked )
.fusion/redline-web/target/release/redline-web --target-bin "\$ENGINE" --db /tmp/demo.redline.db --bind 127.0.0.1:7788

# optional frontend hot-reload (two terminals):
#   ( cd .fusion/redline-web && just dev-server )   # backend :7788
#   ( cd .fusion/redline-web && just dev-web )        # Vite :5173 -> proxies to :7788
\`\`\`

## Freeze these revisions
Copy the resolved SHAs above into \`.fusion/fusion.lock\` (one \`<name> <sha>\`
per line) and re-run \`just fuse\` for a reproducible tree.
EOF

log "wrote $FUSION_DIR/dev.sh and $FUSION_DIR/README.dev.md"
log "done. Next: cat .fusion/README.dev.md   |   ./.fusion/dev.sh build-all"
