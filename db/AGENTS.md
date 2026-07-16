# db/AGENTS.md

<!-- jankurai generated adapter -->
<!-- jankurai agent request v1 sha256:REPLACE_WITH_HASH -->
Read `AGENTS.md` first. Use `.jankurai/JANKURAI_STANDARD.md` as the canonical jankurai standard.
When a user provides a paper, release, implementation, or handoff plan in the conversation, treat that plan as the controlling plan. Do not route such plans through the separate local phase workflow unless the user explicitly names MASTER_PLAN phase work.
Owns `db/`.
Forbidden: application logic, transport routing, and UI concerns.
Proof lane: `migration / constraint tests`.
Use only `bash ops/ci/run-jankurai.sh ...`; governed Jankurai 1.6.11 is verified fail-closed and client self-update is forbidden.
