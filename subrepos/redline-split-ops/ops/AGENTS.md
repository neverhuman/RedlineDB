# Operations surface

Owns CI orchestration, pinned external-tool installation, and the optional
local pre-push hook. Product code, child repository history, production
promotion, and eligibility decisions are forbidden here. Run
`bash ops/ci/quality-gates.sh`; family cutover remains a separate explicit
`just family-ci && just cutover-verify` decision.
