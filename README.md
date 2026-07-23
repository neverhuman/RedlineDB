# Redline split operations

[![jankurai score](https://img.shields.io/badge/jankurai-ratcheted-blue)](agent/jankurai-baseline.json)

[Agent entrypoint](AGENTS.md) · Candidate status: CI and cutover evidence are
reported by the protected `redline-split-ops/required` lane and checksummed
receipts; no production promotion is claimed.

## Quick start

With Rust 1.96.0 and the pinned local security tools installed, run:

```bash
just authority-validate
just check
just security
just score
```

During the `.jain.5` ownership recovery, `authority-validate` proves only the
closed source manifest. `required`, `doctor`, `validate`, lock review, release
readiness, and cutover remain intentionally red until all six `veox/*`
repositories exist, the product checkouts are adopted, and no-waiver
`proof-refresh` replaces the historical lock and creates its physical mirror.

Use [`docs/architecture.md`](docs/architecture.md) for control-plane boundaries,
[`docs/testing.md`](docs/testing.md) for proof routing and rerun commands, and
[`docs/release-process.md`](docs/release-process.md) for the gated release and
rollback sequence.

`redline-split-ops` owns the nested Redline family manifest, lock verification,
clone/update delegation, bounded family CI, and diagnostics. The five product
repositories remain independent Git repositories and are never included in an
umbrella Cargo workspace.

`redline-central` is now the fifth product row at its existing protected
`redline-central-v4.1.0-jain.1` identity. Family CI requires both its protected
`required` lane and its real-service `family-release` lane. The unchanged
historical lock still omits Central and is therefore not release evidence.

The manifest is the sole release-identity authority. Each repository declares
its product version, corrective tag revision, exact tag, canonical local-forge `veox/*` remote,
release commit/tree checksum binding, and protection policy. All five product
pairs are exact; only the unreleased control-plane pair remains `PENDING`.
`proof-refresh` refuses any pending product identity.

```text
jain-redline/
├── redline-split-ops/                 # this repository
│   ├── repos.manifest.toml            # canonical child manifest
│   └── redline.lock.toml              # authoritative child pins and proof lock
├── redline{,-core,-testing,-web}/     # physical standalone repositories
├── redline-central/                   # fifth governed product repository
└── redline.lock.toml                  # transactional compatibility mirror
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
temporary standalone `git clone --no-local` checked out at the exact reviewed
SHA, and writes a JSON receipt, per-repository logs, and a `<receipt>.sha256`
sidecar. Clone roots and Git directories must be physical, independent, full
history repositories with no object alternates or linked-checkout metadata.
The clone sandbox is rooted beneath this repository's `target/` directory.
Cleanup is descriptor- and marker-bound and does not follow symlink targets;
after quarantine it never reopens the sandbox pathname, and the held marker and
root inodes must both be proven unlinked. A present tag that points
anywhere other than the reviewed head is an immutable-tag conflict; a tag may
be absent during this CI step, but `proof-refresh` requires it locally and on
Jeryu.

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
  "engine_tag": "redline-core-v4.1.0-jain.5",
  "engine_commit": "<family-ci redline-core commit>",
  "proof_lock_id": "redline-proof/v2/4.1.0/<family-ci redline-core commit>",
  "family_ci_receipt_sha256": "<family-ci receipt SHA256>",
  "manifest_sha256": "<canonical Redline manifest SHA256>",
  "policy_sha256": "<canonical Redline policy SHA256>",
  "consumer_manifest_sha256": "<consumer manifest SHA256>",
  "consumer_policy_sha256": "<consumer CI policy SHA256>",
  "test_log": "redline-consumer-jain-split.test.log",
  "test_log_sha256": "<fresh consumer test-log SHA256>",
  "tool_version": "jain-redline-consumer/v1"
}
```

The Jeryu receipt uses `consumer: "jeryu-split"` and required check
`jeryu-split/redline-consumer`, plus tool version
`jeryu-redline-consumer/v1`. The manifest, policy, and fresh test-log hashes are
binding inputs rather than descriptive metadata. Each evidence file must have a standard
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

The retained 8.0.0 successor receipts document the superseded `.jain.4`
attempt only. They cannot validate this six-repository `.jain.5` authority or
make its four-row `.jain.3` lock green. Recovery requires repository adoption,
fresh five-product family CI, both fresh consumer receipts, and one normal
no-waiver `proof-refresh`. Generated-evidence ownership and merge-audit
acceptance are documented in `docs/generated-zones.md` and
`docs/audit-rubric.md`.

Reviewed cutover inputs live under `release-evidence/<release>/`, alongside
their checksum sidecars and the family-CI logs named by the receipt. The proof
lock records paths relative to this repository so a clean reviewed checkout can
reconstruct the decision. Temporary or ignored evidence paths are not valid
release inputs.
