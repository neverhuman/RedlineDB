# jain-split-ops ops instructions

The `ops/` tree is the executable control plane for the split family. Before
editing host CI, materialization, or release scripts, read `README.md`,
`SPLIT.md`, and `docs/local-jeryu-forge-agent-workflow.md`.

All generated operational sources must use local Jeryu:
`http://127.0.0.1:8787/git/veox/<repo>.git`. Do not rewrite historical Cargo
pin URLs merely to change the owner spelling; those pins retain their existing
crate identity until an explicit dependency re-pin.

Do not use `~/jeryu-split` as a Jain workspace or source. Do not add GitHub.com
or SSH GitHub as agent-facing sources. Do not use `target/bare-mirrors` as an
agent-facing source; bare mirrors are permitted only inside CI-scoped rewrite
config emitted by `ops/ci/split-host-ci.sh` or lock-generation helpers.

Run `just jeryu-ready` before PR/tag work, `just fast` for syntax checks, and
`just required` before pushing control plane changes.

Closed dependency caches are release authority, not developer cache hints.
Changes to `ops/ci/*-runtime.sh`, a committed cache lock, or cache staging in
`host-ci-sandbox.sh` must bind the exact product lock and platform closure,
validate immutable root custody before copying, and use only the writable
per-request copy inside the network-isolated worker. Never mount an ambient
home-directory cache or make network access a fallback. Run the matching
`*-runtime-test.sh`, `host-ci-integrity-test.sh`, and the privileged
`split-host-ci-integrity-test.sh`; the focused runtime fixture must prove a
fresh install with the network namespace disconnected.
