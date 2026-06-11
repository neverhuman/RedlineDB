# crates/domain — Hub Exception Surface

**Owns:** Typed error definitions and repair manifests for the RedlineDB hub.

**This cell contains no engine source.** Engine code (Rust, SQL) lives in `redline-core`.
This cell holds only the exception/error manifest that documents every failure mode
an agent or operator can encounter when working with this hub.

**Forbidden:** `.rs` files, `Cargo.toml`, schema files, migration SQL.
Those belong in `redline-core`.

**Proof lane:** `bash ops/ci/pr-ci.sh` validates the thin-hub invariant (no `.rs` files
or `Cargo.toml` in this repo). The `exceptions.json` file must remain in sync with the
`ERR_*` codes defined in `ops/ci/lib.sh`.

**Exception manifest:** `exceptions.json`
Each entry declares:
- `purpose` — the error kind (matches `ERR_*` constant in `ops/ci/lib.sh`)
- `reason` — root cause in plain language
- `common_fixes` — ordered list of repair steps
- `docs_url` — local doc anchor with more detail
- `repair_hint` — single command an agent can run to reproduce and diagnose
