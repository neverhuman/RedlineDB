#!/usr/bin/env bash
# Fully local, pinned, fail-closed supply-chain evidence.
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$ROOT_DIR"

readonly CARGO_AUDIT_BIN="/home/ubuntu/.cargo/bin/cargo-audit"
readonly CARGO_AUDIT_SHA256="1a17ff4c0449d1924aacda8dd20c06dccc3cceeed4dd17a71523f672bf97b70b"
readonly CARGO_DENY_BIN="/home/ubuntu/.cargo/bin/cargo-deny"
readonly CARGO_DENY_SHA256="ef27c757f50d77c5c2d9114fbc6ad45d2b8903506cead473a70b8ee659ea7a18"
readonly GITLEAKS_BIN="/home/ubuntu/.cargo/bin/gitleaks"
readonly GITLEAKS_SHA256="50b742abd7daad8bbddb6301f3017efb680632d9a5b3b4d8f137b3aac250e359"
readonly ZIZMOR_BIN="/home/ubuntu/.cargo/bin/zizmor"
readonly ZIZMOR_SHA256="6eabb307e2c0c35aa3b397d35518db66106c6909f86e7e0d7d898bc6587aa7fc"
readonly SYFT_BIN="/home/ubuntu/.local/bin/syft"
readonly SYFT_SHA256="eb9714fb8e4b8f2a647e7bb312f1e0b9f83a7aa30418658bf46583cfa83d27d2"
readonly SYFT_CONFIG="${ROOT_DIR}/ops/ci/syft.yaml"
readonly RUSTSEC_DB="/home/ubuntu/.cargo/advisory-db"
readonly RUSTSEC_DB_COMMIT="9f3e138091487e69144f536d36976e427a7a3307"

for tool in git jq npm sha256sum realpath tar; do
  has "$tool" || fail "required local security tool is missing: $tool"
done
[[ -z "$(git status --porcelain)" ]] || fail "security evidence requires a clean checkout"
jain_verify_exact_executable cargo-audit "$CARGO_AUDIT_BIN" "$CARGO_AUDIT_SHA256" || exit 1
jain_verify_exact_executable cargo-deny "$CARGO_DENY_BIN" "$CARGO_DENY_SHA256" || exit 1
jain_verify_exact_executable gitleaks "$GITLEAKS_BIN" "$GITLEAKS_SHA256" || exit 1
jain_verify_exact_executable zizmor "$ZIZMOR_BIN" "$ZIZMOR_SHA256" || exit 1
jain_verify_exact_executable syft "$SYFT_BIN" "$SYFT_SHA256" || exit 1
[[ -f "$SYFT_CONFIG" && ! -L "$SYFT_CONFIG" ]] \
  || fail "pinned local Syft configuration is missing or symlinked"

[[ -d "$RUSTSEC_DB" && ! -L "$RUSTSEC_DB" \
  && "$(realpath -e -- "$RUSTSEC_DB")" == "$RUSTSEC_DB" \
  && -d "$RUSTSEC_DB/.git" && ! -L "$RUSTSEC_DB/.git" ]] \
  || fail "pinned local RustSec database custody is invalid"
git -C "$RUSTSEC_DB" cat-file -e "${RUSTSEC_DB_COMMIT}^{commit}" \
  || fail "pinned local RustSec commit object is unavailable"

export CARGO_NET_OFFLINE=true
export SYFT_CHECK_FOR_APP_UPDATE=false
artifact_root="${ARTIFACT_DIR}/security"
mkdir -p "$artifact_root"
rustsec_archive="${artifact_root}/rustsec-db.tar"
rustsec_snapshot="${artifact_root}/rustsec-db"
rm -rf -- "$rustsec_snapshot"
mkdir -p "$rustsec_snapshot"
git -C "$RUSTSEC_DB" archive --format=tar \
  --output "$rustsec_archive" "$RUSTSEC_DB_COMMIT"
rustsec_archive_sha256="$(jain_sha256 "$rustsec_archive")"
tar -xf "$rustsec_archive" -C "$rustsec_snapshot"
[[ -z "$(find "$rustsec_snapshot" -type l -print -quit)" ]] \
  || fail "pinned RustSec snapshot contains a symlink"
rustsec_tree="$(git -C "$RUSTSEC_DB" rev-parse "${RUSTSEC_DB_COMMIT}^{tree}")"

log "security: pinned local gitleaks"
"$GITLEAKS_BIN" detect --source . --config gitleaks.toml --no-banner --redact \
  --report-format json --report-path "$artifact_root/gitleaks.json"
jq -e 'type == "array" and length == 0' "$artifact_root/gitleaks.json" >/dev/null

log "security: pinned local RustSec database"
"$CARGO_AUDIT_BIN" audit --no-fetch --db "$rustsec_snapshot" --deny warnings --json \
  >"$artifact_root/cargo-audit.json"
jq -e '.vulnerabilities.found == false and (.vulnerabilities.list | length) == 0
  and ((.warnings // {}) | length == 0)' "$artifact_root/cargo-audit.json" >/dev/null

log "security: pinned local cargo-deny"
"$CARGO_DENY_BIN" check --disable-fetch --deny warnings \
  >"$artifact_root/cargo-deny.log" 2>&1

log "security: offline npm advisory cache"
(cd "$WEB_DIR" && npm audit --offline --audit-level=high --json) \
  >"$artifact_root/npm-audit.json"
jq -e '.metadata.vulnerabilities.high == 0
  and .metadata.vulnerabilities.critical == 0' "$artifact_root/npm-audit.json" >/dev/null

log "security: pinned offline blocking zizmor"
"$ZIZMOR_BIN" --offline --format json --no-progress .github/workflows \
  >"$artifact_root/zizmor.json"
jq -e 'type == "array" and length == 0' "$artifact_root/zizmor.json" >/dev/null

log "security: pinned update-disabled local Syft SBOM"
"$SYFT_BIN" scan dir:. --config "$SYFT_CONFIG" -q \
  --exclude './.git/**' --exclude './target/**' \
  --source-name redline-web --source-version "$(git rev-parse HEAD)" \
  --output "spdx-json=$artifact_root/redline-web.spdx.json"
jq -e '.spdxVersion | startswith("SPDX-")' \
  "$artifact_root/redline-web.spdx.json" >/dev/null

jq -n \
  --arg commit "$(git rev-parse HEAD)" \
  --arg tree "$(git rev-parse 'HEAD^{tree}')" \
  --arg rustsec_commit "$RUSTSEC_DB_COMMIT" \
  --arg rustsec_tree "$rustsec_tree" \
  --arg rustsec_archive_sha256 "$rustsec_archive_sha256" \
  --arg cargo_audit_sha256 "$CARGO_AUDIT_SHA256" \
  --arg cargo_deny_sha256 "$CARGO_DENY_SHA256" \
  --arg gitleaks_sha256 "$GITLEAKS_SHA256" \
  --arg zizmor_sha256 "$ZIZMOR_SHA256" \
  --arg syft_sha256 "$SYFT_SHA256" \
  '{schema_version:"redline.web.security/v1",status:"pass",network:"offline",
    commit:$commit,tree:$tree,rustsec_db_commit:$rustsec_commit,
    rustsec_db_tree:$rustsec_tree,rustsec_db_archive_sha256:$rustsec_archive_sha256,
    tool_sha256:{cargo_audit:$cargo_audit_sha256,cargo_deny:$cargo_deny_sha256,
      gitleaks:$gitleaks_sha256,zizmor:$zizmor_sha256,syft:$syft_sha256},
    checks:["gitleaks","cargo-audit-pinned-no-fetch","cargo-deny-no-fetch",
      "npm-audit-offline","zizmor-offline-strict","syft-pinned-update-disabled"]}' \
  >"$artifact_root/source-evidence.json"
log "security: complete"
