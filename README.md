# Redline split operations

[![jankurai score](https://img.shields.io/badge/jankurai-ratcheted-blue)](agent/jankurai-baseline.json)

[Agent entrypoint](AGENTS.md) · Candidate status: CI and cutover evidence are
reported by the protected `redline-split-ops/required` lane and checksummed
receipts; no production promotion is claimed.

## Quick start

With Rust 1.96.0 and the pinned local security tools installed, run:

```bash
just required
just check
just security
just score
```

Use [`docs/architecture.md`](docs/architecture.md) for control-plane boundaries,
[`docs/testing.md`](docs/testing.md) for proof routing and rerun commands, and
[`docs/release-process.md`](docs/release-process.md) for the gated release and
rollback sequence.

`redline-split-ops` owns the nested Redline family manifest, lock verification,
clone/update delegation, bounded family CI, and diagnostics. The four child
repositories remain independent Git repositories and are never included in an
umbrella Cargo workspace.

`redline-central` has a separate protected onboarding review and is not active
in the current manifest or lock. The parser already reserves its exact initial
release identity, and family CI requires both its protected `required` lane and
its real-service `family-release` lane whenever the governed row is present.
Adding that row remains a protected authority change after Central lands.

The manifest is the sole release-identity authority. Each repository declares
its product version, corrective tag revision, exact tag, Jeryu remote,
release commit/tree checksum binding, and protection policy. Commit/checksum
pairs may both be `PENDING` while review is underway; `proof-refresh` refuses
them until both are exact.

```text
jain-redline/
├── redline-split-ops/                 # this repository
│   ├── repos.manifest.toml            # canonical child manifest
│   └── redline.lock.toml              # authoritative child pins and proof lock
├── redline{,-core,-testing,-web}/     # physical standalone repositories
├── redline-central/                   # governed support; active row awaits landing
└── redline.lock.toml                  # transactional compatibility mirror
```

The commands accept `REDLINE_SPLIT_ROOT` for copied or relocated checkouts:

```bash
REDLINE_SPLIT_ROOT="$PWD" ./redlinectl validate
./redlinectl clone --dry-run
./redlinectl family-ci --receipt target/release-evidence/redline-family-ci.json
./redlinectl offline-containment \
  --repo redline-testing \
  --cargo-home /home/ubuntu/jain-split/target/redline-testing-cargo-home \
  --receipt target/release-evidence/redline-testing-offline-containment.json
```

`redlinectl` is a small shell launcher for the standalone Rust control-plane
binary in `tools/redline-proof/src/main.rs`. This repository is a Rust package for operational
tooling, but it is deliberately not a Cargo workspace and is never a Redline
product or dependency member. All proof, lock, receipt, and safety tests run as
Rust tests with `cargo test --locked`; Python is reserved for cross-language
parity harnesses and is not used by this control plane.

`offline-containment` is a closed-input proof for one exact released
repository. It clones the manifest-bound commit directly from local Jeryu with
`git clone --no-local`, detaches it, removes the remote, recursively rejects
symlinks/special nodes, hashes in-tree Cargo custody and `Cargo.lock`, then runs
`cargo test --workspace --locked --offline` in a user systemd scope with
`IPAddressDeny=any` and `RestrictAddressFamilies=AF_UNIX`. Any failure leaves no
passing receipt, and the standalone sandbox is automatically removed.

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
  "engine_tag": "redline-core-v4.1.0-jain.4",
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

When a reviewed Core correction advances exactly one immutable tag revision,
start the transition before creating that tag:

```bash
./redlinectl proof-refresh --prepare-successor \
  --receipt release-evidence/8.0.0/redline-proof-successor-jain4-prepared.json
```

This mode accepts no family/consumer evidence or eligibility override. It
requires clean forge-equal child mains, the proposed Core tag to be absent, and
the byte-exact reviewed predecessor lock bound by SHA-256 in the manifest. It
writes only the candidate authoritative lock, its sidecar, and a typed
`prepared` receipt; the compatibility mirror stays unchanged. The manifest,
historical authoritative lock, and preparation receipt land through protected
review first. The review lane accepts that split state only when the historical
bytes are the tool's exact manifest-bound rendering; operational `validate`,
`lock-verify`, and `cutover-verify` remain strict and fail on the split state.
`successor-receipt-verify` checksum-verifies the closed receipt fields, both
canonical lock paths, predecessor/prepared digests, and exact engine identities.
Generated-evidence ownership and merge-audit acceptance are documented in
`docs/generated-zones.md` and `docs/audit-rubric.md`.
After merge, reconcile once:

```bash
./redlinectl proof-refresh --reconcile-successor \
  --receipt release-evidence/8.0.0/redline-proof-successor-jain4-reconciled.json
```

This atomically updates the mirror and produces the governed `reconciled`
receipt, which is then submitted through protected review. Any other byte drift
fails closed. Only a normal two-consumer `proof-refresh` can restore cutover
eligibility.

Reviewed cutover inputs live under `release-evidence/<release>/`, alongside
their checksum sidecars and the family-CI logs named by the receipt. The proof
lock records paths relative to this repository so a clean reviewed checkout can
reconstruct the decision. Temporary or ignored evidence paths are not valid
release inputs.
