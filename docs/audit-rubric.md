# Audit acceptance

The protected lane runs the pinned full Jankurai audit through `just required`.
Acceptance requires the configured minimum score, zero hard findings, zero
caps, clean-worktree evidence, security scans, coverage evidence, and the
release-readiness receipt.

Local repair loops may run `bash ops/ci/jankurai.sh diff-audit . --base-ref <reviewed-base>`,
but changed-fast output never replaces the full audit. Any new cap blocks the
PR until the changed surface includes the missing test, release, ownership, or
agent-readable documentation evidence; operators do not waive or manually
rewrite the result.

Exact file-to-lane ownership is machine-readable in `agent/owner-map.json` and
`agent/test-map.json`. Generated evidence boundaries are documented in
`docs/generated-zones.md`.
