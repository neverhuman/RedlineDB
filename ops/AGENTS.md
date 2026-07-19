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
