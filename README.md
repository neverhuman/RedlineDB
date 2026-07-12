# Redline split operations

[Agent entrypoint](AGENTS.md) · Candidate status: CI and cutover evidence are
reported by the protected `redline-split-ops/required` lane and checksummed
receipts; no production promotion is claimed.

## Quick start

With Rust 1.96.0 and the pinned local security tools installed, run:

```bash
just check
just security
just score
```

`redline-split-ops` owns the nested Redline family manifest, lock verification,
clone/update delegation, bounded family CI, and diagnostics. The four child
repositories remain independent Git repositories and are never included in an
umbrella Cargo workspace.

The manifest is the sole release-identity authority. Each repository declares
its product version, corrective tag revision, exact tag, Jeryu remote,
release commit/tree checksum binding, and protection policy. Commit/checksum
pairs may both be `PENDING` while review is underway; `proof-refresh` refuses
them until both are exact.

```text
redline-split-ops/              # this repository
redline-split-ops/repos.manifest.toml # canonical child manifest
redline-split-ops/redline.lock.toml # authoritative child pins and proof lock
redline-split/{redline,redline-core,redline-testing,redline-web}/
```

The commands accept `REDLINE_SPLIT_ROOT` for copied or relocated checkouts:

```bash
REDLINE_SPLIT_ROOT="$PWD" ./redlinectl validate
./redlinectl clone --dry-run
./redlinectl family-ci --receipt target/release-evidence/redline-family-ci.json
```

`redlinectl` is a small shell launcher for the standalone Rust control-plane
binary in `tools/redline-proof/src/main.rs`. This repository is a Rust package for operational
tooling, but it is deliberately not a Cargo workspace and is never a Redline
product or dependency member. All proof, lock, receipt, and safety tests run as
Rust tests with `cargo test --locked`; Python is reserved for cross-language
parity harnesses and is not used by this control plane.

Jain and Jeryu delegate here through their nested-family commands. Redline
remains an independent family; both consumers must pin the same engine commit
and proof-lock identity. The cutover command fails closed while parity or
consumer evidence is historical.

## Receipt-driven cutover

`family-ci` refuses dirty or non-`main` checkouts and requires every local head
to equal the forge `main` head. It runs each repository's strict CI from a
detached worktree and writes a JSON receipt, per-repository logs, and a
`<receipt>.sha256` sidecar. A present tag that points anywhere other than the
reviewed head is an immutable-tag conflict; a tag may be absent during this CI
step, but `proof-refresh` requires it locally and on Jeryu.

The runner removes the control plane's Rust toolchain override from every child
command. It also builds Redline Testing from its reviewed commit and feeds Core
only the staged `file://` release package, checksum, and manifest. The Core row
binds that package, binary, manifest, source commit, and build log by SHA-256.

Jain and Jeryu each provide a fresh checksummed JSON object with exactly these
fields (replace the values with their reviewed consumer check output):

```json
{
  "schema_version": "redline.consumer-evidence/v1",
  "consumer": "jain-split",
  "family": "redline-split",
  "generated_at": "2026-07-12T12:00:00Z",
  "status": "pass",
  "source_commit": "0123456789abcdef0123456789abcdef01234567",
  "required_check": "jain-split/redline-consumer",
  "engine_tag": "redline-core-v4.1.0-jain.2",
  "engine_commit": "<family-ci redline-core commit>",
  "proof_lock_id": "redline-proof/v2/4.1.0/<family-ci redline-core commit>",
  "family_ci_receipt_sha256": "<family-ci receipt SHA256>"
}
```

The Jeryu receipt uses `consumer: "jeryu-split"` and required check
`jeryu-split/redline-consumer`. Each evidence file must have a standard
`<file>.sha256` sidecar containing `<digest>  <basename>`. Evidence older than
24 hours, future-dated evidence, unknown fields, manual booleans, mismatched
commits, stale logs, or changed checksums are rejected.

```bash
./redlinectl proof-refresh \
  --family-ci target/release-evidence/redline-family-ci.json \
  --jain-evidence /path/to/jain-redline-consumer.json \
  --jeryu-evidence /path/to/jeryu-redline-consumer.json \
  --receipt target/release-evidence/redline-proof-refresh.json
./redlinectl cutover-verify
```

`proof-refresh` accepts no eligibility flag. It derives the proof hashes,
proof-lock identity, tag metadata, and `cutover_eligible` value, then safely
replaces the authoritative lock and its compatibility mirror with identical
bytes. `cutover-verify` reconstructs the lock from the still-fresh receipts and
live immutable tag readback; a manual lock edit cannot make cutover pass.
