# jankurai Standard Agent Bootstrap

Standard version: `0.8.0`

Use `.jankurai/owner-map.json`, `.jankurai/test-map.json`,
`.jankurai/generated-zones.toml`, `.jankurai/proof-lanes.toml`,
`.jankurai/audit-policy.toml`, and `.jankurai/unsafe-ledger.toml` before
editing.

## Notes for redline-testing

* This repo is a satellite that produces signed reference test artifacts for
  RedlineDB CI. It does NOT itself implement product behavior.
* The audit policy is advisory; CI does not gate on jankurai findings. The
  registry of decisions lives in `audit-policy.toml`.
* SQL fixtures live under `corpus/`; integration tests under `tests/` consume
  them via `include_str!`. Both areas are excluded from product-language
  scanning so `sql-bad-behavior` does not fire on intentionally-dense
  fixture SQL.
* The ship contract: a test case lands on `main` only after passing its
  reference self-compare (`sqlite3↔sqlite3` for parity cases, `psql↔psql`
  for beyond-SQLite cases). `xtask ship-gate` enforces this mechanically.
