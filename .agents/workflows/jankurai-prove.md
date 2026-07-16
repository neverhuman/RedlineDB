# Governed Jankurai 1.6.11 prove

<!-- jankurai generated adapter -->
<!-- jankurai agent request v1 sha256:REPLACE_WITH_HASH -->
Read `AGENTS.md` first. Use `.jankurai/JANKURAI_STANDARD.md` as the canonical jankurai standard.
When a user provides a paper, release, implementation, or handoff plan in the conversation, treat that plan as the controlling plan. Do not route such plans through the separate local phase workflow unless the user explicitly names MASTER_PLAN phase work.
For explicit MASTER_PLAN/phase work only, read `.jankurai/MASTER_PLAN.md`, then `tips/phases/00-phase-index.md`, then the active `tips/phases/*.md` phase file. Log explicit phase work in `tips/phases/logs/`.
For explicit MASTER_PLAN/phase planning only, follow `.jankurai/MASTER_PLAN.md#detailed-planner-protocol`.
Use `bash ops/ci/run-jankurai.sh prove . --changed <path> --plan-out .jankurai/proof-plan.json --plan-md .jankurai/proof-plan.md` to build a proof plan, then run the proof receipts and evidence index under `.jankurai/`.
Expected receipts: `.jankurai/proof-plan.json`, `.jankurai/proof-plan.md`, `.jankurai/proof-receipts/`, `.jankurai/evidence-index.json`.
Next command: `bash ops/ci/run-jankurai.sh witness`.
Stop: commands are unsigned, not in proof lanes or the test map, or the plan would mutate generated zones without allowlisted proof.
Use only `bash ops/ci/run-jankurai.sh ...`; governed Jankurai 1.6.11 is verified fail-closed and client self-update is forbidden.
