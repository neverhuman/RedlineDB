@/home/ubuntu/.codex/RTK.md

# Operations ownership

`ops/ci/` owns local proof scripts. Every score invocation routes through
`ops/ci/jankurai.sh`; do not add ambient Jankurai selection or install fallback.
Run `rtk bash scripts/ci-local.sh required` after changing this directory.
