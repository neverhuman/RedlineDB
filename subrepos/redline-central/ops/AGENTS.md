@/home/ubuntu/.codex/RTK.md

# Operations ownership

`ops/ci/` owns local proof scripts. Every score invocation routes through
`ops/ci/jankurai.sh`; freeze the root-controlled release PATH selection and do
not add an install or unverified fallback.
Run `rtk bash scripts/ci-local.sh required` after changing this directory.
