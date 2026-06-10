# Release process

## Version source

The single version source is `apps/api/Cargo.toml` (`package.version`), mirrored
by `apps/web/package.json`. Bump both together; record the change in
[`CHANGELOG.md`](../CHANGELOG.md).

## Cutting a release

1. Update `CHANGELOG.md` (move `Unreleased` items under the new version + date).
2. Bump `version` in `apps/api/Cargo.toml` and `apps/web/package.json`.
3. Validate: `bash ops/ci/pr-ci.sh` (fast + web + backend + security + e2e +
   `ops/ci/security.sh` + `ops/ci/release-readiness.sh` + jankurai evidence).
4. Build the artifact: `cargo build --release --locked` — a single self-contained
   binary that embeds `apps/web/dist`.
5. Land via a jeryu PR / GitLab MR (MR-only; never push to `main`). CI on the PR
   is the gate.

## Integrity / provenance

- `Cargo.lock` and `apps/web/package-lock.json` pin the dependency graph.
- `ops/ci/security.sh` runs gitleaks, cargo-audit, cargo-deny, npm audit, and
  generates an SPDX SBOM under `.artifacts/security/`.
- Every external GitHub Action is pinned to a 40-hex commit SHA.
- Release evidence receipts: `target/jankurai/release-readiness.json` and
  `target/jankurai/cost-budget.json`.

## Rollback

The release is a single binary with no migrations and no durable server-side
state (the database is supplied at runtime via `--db`/`--target-bin`), so
rollback is: redeploy the previous tagged binary and restart. No data migration
or down-script is required. If a bad build shipped, revert the version bump
commit, re-run `bash ops/ci/pr-ci.sh`, and rebuild.
