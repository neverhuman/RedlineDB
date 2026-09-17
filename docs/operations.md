# Operations

`redline-testing` is an offline CLI conformance harness. It has no long-running
service, no database of its own, and no network dependency at runtime — it
drives local `sqlite3` / `psql` subprocess shells and writes JSONL + artifacts.

## Monitoring

- **Artifact drift**: `cargo test --locked --test release_manifest_integrity`
  recomputes every bundled file's SHA-256 against `release-manifest.json`.
- **Score drift**: `jankurai audit` writes `.jankurai/score-history.jsonl`; the
  Rust `xtask update-badge` command keeps the README badge synchronized.
- **Downstream**: RedlineDB CI consumes the pinned tarball and surfaces any
  conformance regression against the published corpus.

## Backups

Every shipped artifact is content-addressed. The release tarball plus its
`.sha256` sidecar and `release-manifest.json` are the immutable backup of a
release; old release tags are never moved.

## Rollback

A release tag is immutable. To roll back, publish a higher version that
restores prior behavior and ask the RedlineDB team to bump their pin (see
[`docs/release.md`](release.md#rollback)).

## Abuse controls and least privilege

- The runner executes only allowlisted subprocess binaries (`sqlite3`, `psql`,
  the target CLI) with bounded `Duration` timeouts.
- No untrusted network input is accepted; inputs are developer-controlled
  corpus shards validated by `xtask ship-gate`.
- CI tokens are least-privilege: the default job is `contents: read`; only the
  badge job takes `contents: write`, only the release job takes attestation
  scopes. Every GitHub Action is pinned to a 40-hex commit SHA.

## Kill switch

`agent/cost-budget.toml` declares `kill_switch_env =
"REDLINE_TESTING_KILL_SWITCH"`; all paid quota caps and stop conditions are
zero / fail-closed by default.
