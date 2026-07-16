# Use governed Jankurai

This repository never installs or self-updates Jankurai. Run
`bash ops/ci/run-jankurai.sh init --profile rust-ts-postgres --ide all --mode advisory --dry-run`,
review the plan, then rerun the same governed wrapper with `--yes`. The wrapper accepts only the
content-addressed Jankurai 1.6.11 identity.

For Rust services that want runtime repair packets, an optional `witness-rt` crate can emit packets that feed the Rust witness and diagnose flows.
