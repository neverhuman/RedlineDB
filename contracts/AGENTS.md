<!-- jankurai generated adapter -->
<!-- jankurai agent request v1 sha256:REPLACE_WITH_HASH -->
Read `AGENTS.md` first. Use `.jankurai/JANKURAI_STANDARD.md` as the canonical jankurai standard.
Owns `contracts/`.
Forbidden: handwritten transport glue, generated clients, and product truth.
Proof lane: `contract drift check` via `bash ops/ci/contract-drift.sh`.
If jankurai is installed, run `jankurai update --client-start --quiet` before work; do not apply updates unless the user asks.
