#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

fail() {
    printf 'release lane interface: %s\n' "$*" >&2
    exit 1
}

dispatcher="scripts/ci-local.sh"
for route in security score contract-drift artifact-support; do
    grep -Fq "  ${route}) exec bash \"\$repo_root/ops/ci/" "$dispatcher" \
        || fail "dispatcher is missing the explicit $route route"
done

for lane in \
    ops/ci/security.sh \
    ops/ci/score.sh \
    ops/ci/contract-drift.sh \
    ops/ci/artifact_support.sh
do
    if [ ! -f "$lane" ] || [ -L "$lane" ] || [ ! -x "$lane" ]; then
        fail "$lane must be a physical executable file"
    fi
done

score_lane="$(cat ops/ci/score.sh)"
for invariant in \
    '--mode ratchet' \
    '--no-score-history' \
    '.jankurai/baselines/main.repo-score.json' \
    '.score >= 85' \
    '.decision.hard_findings == 0' \
    '.decision.ratchet.allowed_drop == 0' \
    '.decision.status == "pass"' \
    '.decision.passed == true' \
    '.decision.ratchet.passed == true' \
    '.decision.ratchet.policy_changed == false' \
    'main.policy-transition.json'
do
    grep -Fq -- "$invariant" <<<"$score_lane" \
        || fail "score lane lost invariant: $invariant"
done

contract_lane="$(cat ops/ci/contract-drift.sh)"
grep -Fq 'status == "artifact_verified"' <<<"$contract_lane" \
    || fail "contract-drift must require artifact-verified evidence"
grep -Fq 'redline.contract-drift/v1' <<<"$contract_lane" \
    || fail "contract-drift receipt schema is missing"

security_lane="$(cat ops/ci/security.sh)"
security_wrapper="$(cat tools/security-lane.sh)"
for forbidden in \
    '|| true' \
    ci_soft_gate
do
    ! grep -Fq -- "$forbidden" <<<"$security_lane" \
        || fail "security contains fail-open control: $forbidden"
    ! grep -Fq -- "$forbidden" <<<"$security_wrapper" \
        || fail "security wrapper contains fail-open control: $forbidden"
done
for invariant in \
    'cargo audit' \
    'cargo deny --all-features check' \
    'gitleaks detect' \
    'just security-sbom' \
    'just security-workflows'
do
    grep -Fq -- "$invariant" <<<"$security_lane" \
        || fail "security lane lost hard-gate invariant: $invariant"
done
grep -Fq 'bash "$ROOT/ops/ci/security.sh"' <<<"$security_wrapper" \
    || fail "security wrapper must delegate to the hard-gated security lane"
! grep -Fq 'dependency-review.sh' <<<"$security_wrapper" \
    || fail "security wrapper must not add a fail-open dependency-review route"

artifact_lane="$(cat ops/ci/artifact_support.sh)"
for forbidden in \
    SIGNRAIL \
    ED25519_SEED \
    'cargo install' \
    'https://github.com' \
    sign-release
do
    ! grep -Fq -- "$forbidden" <<<"$artifact_lane" \
        || fail "artifact-support contains forbidden review-lane authority: $forbidden"
done
grep -Fq 'gzip -n' <<<"$artifact_lane" \
    || fail "artifact-support must normalize gzip metadata"
grep -Fq -- "--mode='u+rwX,go+rX,go-w,a-s'" <<<"$artifact_lane" \
    || fail "artifact-support must normalize archive modes"
for invariant in \
    'git clone --no-local --no-hardlinks --no-tags' \
    'build_count: 2' \
    'logs/build-1.log' \
    'logs/build-2.log'
do
    grep -Fq -- "$invariant" <<<"$artifact_lane" \
        || fail "artifact-support lost independent-build invariant: $invariant"
done
! grep -Fq 'install -m 0644 "$out_dir/logs/' <<<"$artifact_lane" \
    || fail "artifact-support must not copy a prior log into an alleged build"

printf 'release lane interface: pass\n'
