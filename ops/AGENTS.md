# ops/AGENTS.md

<!-- jankurai generated adapter -->
<!-- jankurai agent request v1 sha256:REPLACE_WITH_HASH -->
Read `AGENTS.md` first. Use `.jankurai/JANKURAI_STANDARD.md` as the canonical jankurai standard.
When a user provides a paper, release, implementation, or handoff plan in the conversation, treat that plan as the controlling plan. Do not route such plans through the separate local phase workflow unless the user explicitly names MASTER_PLAN phase work.
Owns `ops/`, including the standard `score`, `contract-drift`, and unsigned
`artifact-support` release evidence lanes.
Forbidden: product feature code, domain policy, and direct DB writes.
Proof lanes: `scripts/ci-local.sh required`, `score`, `contract-drift`,
`artifact-support`, and the security/workflow lint surface.
If jankurai is installed, run `jankurai update --client-start --quiet` before work; do not apply updates unless the user asks.
