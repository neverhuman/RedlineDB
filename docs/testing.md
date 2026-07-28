# Testing

Use the local lanes:

```bash
just fast
just required
just score
```

`fast` checks shell syntax and Python compilation. `required` adds shellcheck
when available, materializer tests, manifest parsing, and local-Jeryu policy
validation. `score` runs Jankurai with the committed baseline ratchet.

For family-level proof, run each changed member repo's `just required` and
`just score` from its own checkout, then post statuses through
`ops/ci/split-host-ci.sh`.

## Host-CI boundary tests

Changes to `ops/ci/host-ci-sandbox.sh` or `ops/ci/split-host-ci.sh` require both
boundary layers:

```bash
bash ops/ci/host-ci-integrity-test.sh
bash ops/ci/split-host-ci-integrity-test.sh
```

The first is a fast structural contract test. The second is the privileged
adversarial integration matrix: it installs disposable reviewed broker bytes,
uses standalone no-local Git materialization, creates isolated PID/user/mount
and network namespaces, exercises the worker, seals evidence, and verifies
one-shot publication. Its temporary state is removed automatically.

Release-authority coverage proves both success and hostile cases. The success
probe checks the exact eight-key projection, control commit, committed manifest
digest, candidate fields, root ownership, `0444` mode, single link, and
root-supplied SHA-256. It also proves worker writes and renames are blocked.
Crafted parent requests carrying either `JAIN_RELEASE_AUTHORITY_PROJECTION` or
`JAIN_RELEASE_AUTHORITY_PROJECTION_SHA256` must be rejected before a worker or
forge request starts. The Jain Ops consumer fixture additionally rejects a
missing path, symlink, wrong custody or checksum, malformed/extra fields, and
wrong release, status, formal-GA, or rollback values.

The Redline Web npm authority has a focused lane:

```bash
bash ops/ci/npm-runtime-test.sh
bash ops/ci/host-ci-integrity-test.sh
bash ops/ci/split-host-ci-integrity-test.sh
```

The runtime fixture builds two deterministic npm tarballs, derives a closed
cache, validates and stages it, and runs `npm ci --offline` inside a new network
namespace (or inside the already-isolated worker). It must also reject a changed
lock, unknown authority fields, the wrong platform, missing or extra cache
objects, tampered content, symlinks, hardlinks, a re-sealed wrong SHA-512, and
a re-sealed URL-index mismatch. The integration matrix proves the same source
is part of the root-owned publisher boundary. A local full lane may separately
report foreign live-checkout census failures; do not change another checkout
to make that census green.

## Sealed release verification

Host-local `just required` checks the live checkout/remotes census and may
correctly fail when another claimed repository is dirty, unmanaged, or pending
custody. That is an external preflight result, not permission to skip a test or
rewrite another agent's state. The authoritative PR check runs the same source
from the configured root-sealed host-CI parent. There the worker validates the
authenticated outer projection and does not treat mutable host checkout remotes
as source authority.

Before publication, run full and true `--changed-fast` Jankurai against the
exact protected base. Both reports must pass the policy floor and ratchet with
zero caps and zero hard findings. A full score cannot hide a changed-fast cap.
After source review, one fresh root-sealed run must publish a common-root
`jankurai/proof` and `<repo>/required` pair for the exact PR head; stale or
mixed-attempt checks are not reusable.

## Release-candidate launch gates

The 8.0.1 result remains a release candidate until a separately authorized
production promotion. A formal launch gate must fail closed unless its raw
receipts prove all of the following:

- security scans, SBOM, provenance, signatures, and image inspection;
- backup and restore readiness for persistent state;
- monitoring for the CLI, web, Redline persistence, and SmartCluster daemon,
  client, and worker health;
- rollback rehearsal to the known `7.0.6` target;
- rate limit and abuse controls for uploads, storage, public routes, and
  bounded cleanup.

The AtomicSoul candidate commands are dry-run only. The budget, quota, spend
cap, stop condition, and kill switch policy is explicit: they set
`ATOMICSOUL_PUSH=0`, cannot change Caddy or production aliases, and use the
following cost budget: zero paid production operations, zero external compute,
and zero image pushes. Any attempted external write stops the run. Missing evidence keeps
`formal_ga = false`; it must never be restated as a passed gate.

## Repair Receipts

Every failing lane should leave a repair receipt or raw artifact under
`target/jankurai/`, `.jankurai/`, `target/security/`, or
`target/artifact-support/`. The receipt should include:

- `purpose`: the control-plane invariant being proven.
- `reason`: why the lane failed or why the artifact was emitted.
- `repair_hint`: the narrowest next command or file to inspect.
- `docs_url`: the local document that explains the lane.
- `common fixes`: rerun `just fast`, then `just required`, then `just score`
  after changing split generator, manifest, or CI files.
- `rerun command`: the exact `just ...` command that reproduces the failure.

## Verifying an authority-bind evidence record

Each `docs/release-evidence/10.0.0/authority-binds-*.md` row is mechanically
checkable; a reviewer verifies rather than trusts it:

1. `git ls-remote http://127.0.0.1:8787/git/<owner>/<member>.git refs/heads/main`
   must equal the recorded release commit.
2. `git ls-remote ... refs/tags/<immutable tag>` must resolve to the same commit.
3. In the member checkout: `git rev-parse <commit>^{tree}` must equal the
   recorded tree, and `git archive --format=tar <commit> | sha256sum` the
   recorded archive digest — the same derivation `splitctl` applies in its
   exactness check, so the comparison is like for like.
4. The named route identities must appear on the coordination board with the
   author, reviewer, approver, and merger all distinct.

A row that fails any step is evidence of a stale or wrong bind, exactly the
class the jain-web split.2→split.4 refresh corrected on 2026-07-28.
