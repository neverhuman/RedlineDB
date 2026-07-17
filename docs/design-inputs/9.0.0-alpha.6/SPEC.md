# Jain compute/storage production-canary requirements

Authority: `authority/production-compute-storage.program-release.toml`  
Evidence index: `docs/release-evidence/9.0.0-alpha.6/evidence-index.json`

This is the stable requirement-ID surface for the compute/storage canary. It narrows the three
custodied merger inputs without weakening them. A requirement passes only through a digest-bound
receipt routed by the authority and evidence index. Anything outside this cut remains explicit and
cannot be inferred from a passing canary.

## Governance and custody

- `A6-GOV-001`: Preserve every merger input byte-identically and verify source, copy, size, and
  SHA-256 before evaluating release state.
- `A6-GOV-002`: Keep the future-major authority side by side with the current release authority;
  neither manifest may be used as the other.
- `A6-GOV-003`: Every release writer derives release names, paths, groups, repositories, and legacy
  firewalls from an explicit authority manifest; product release versions are never Rust constants.
- `A6-GOV-004`: A future-major writer rejects lexical traversal, symlink aliases, protected-release
  paths, protected manifests, and protected tag fragments before creating a file.
- `A6-GOV-005`: All changed surfaces require applicable obligations, exact-head signed
  `jankurai/proof`, score at least 85, hard findings zero, and green required, security,
  compatibility, coverage, and tool-adoption gates.
- `A6-GOV-006`: New repositories enter through typed local-Jeryu creation, protected linear review,
  immutable source custody, provenance maps, exact-head CI, and immutable tags.
- `A6-GOV-007`: Git worktrees are forbidden. Exact-SHA isolation uses automatically removed
  standalone non-local clones.
- `A6-GOV-008`: `jain-fabric` is dependency-free contract authority; `jain-platform` owns signed
  adapters; `jain-shard` is a daemon-free storage engine.

## WorkUnit execution

- `A6-EXE-001`: A signed, ledgered WorkUnit is the only live path for training, inference,
  research, coding, storage, and maintenance execution.
- `A6-EXE-002`: Existing V1 Postcard bytes and priority discriminants remain byte-identical.
- `A6-EXE-003`: A separately negotiated node envelope carries WorkUnit offers and typed ACK or
  rejection frames bound to the exact WorkUnit digest and ControlPosition.
- `A6-EXE-004`: Legacy node protocol cannot advertise eligibility for or execute future-major work.
- `A6-EXE-005`: Hub admission atomically persists Run, Attempt, ledger row, template resolution,
  budget reservation, admission receipt, fenced lease, WorkUnit bytes/digest, control sequence,
  and send outbox.
- `A6-EXE-006`: A node persists exact WorkUnit bytes/digest, attempt fence, issuer trust, and last
  accepted hub epoch/sequence before ACK or launch. Exact replay is idempotent; stale or conflicting
  replay fails closed.
- `A6-EXE-007`: No executable byte runs before the trusted stopped-child gate establishes its
  cgroup, device, mount, Landlock, descriptor, and fence policy.
- `A6-EXE-008`: Nodes use an exact digest-pinned, separately signed adapter registry; unsupported
  templates fail before lease issuance and never fall back to inline compute.
- `A6-EXE-009`: Logs and outputs use bounded governed pipes and streaming Shard manifests without
  silent truncation or whole-output buffering.
- `A6-EXE-010`: Restart-safe, checkpoint-capable, and freeze-safe remain independent capabilities.
- `A6-EXE-011`: Crash matrices prove no unledgered launch and no acknowledged mutation loss across
  every persist, send, ACK, and start boundary.
- `A6-EXE-012`: Cleanup failure quarantines placement and emits durable evidence; it never silently
  restores capacity.

## Composition-owned adapters and Git

- `A6-ADP-001`: Training uses real Jain training logic, immutable Shard I/O, Batch priority, and
  verified checkpoints with no CPU fallback for GPU-required work.
- `A6-ADP-002`: Tabular and generative inference bind exact signed model digests and support bounded
  one-shot and reserved long-running service templates.
- `A6-ADP-003`: Hosted providers remain behind the hub gateway; secrets use protected descriptors,
  never argv.
- `A6-ADP-004`: R&D supports governed interactive sandboxes and durable batch/fan-out work. Ad hoc
  defaults to Batch; unattended fan-out defaults to Opportunistic; host execution fails closed.
- `A6-ADP-005`: Coding sessions use automatically removed `git clone --no-local` exact-base
  checkouts with no agent forge credentials, pushes, hook changes, or grant escape.
- `A6-ADP-006`: Jeryu alone constructs commits, validates paths and freshness, CAS-advances
  namespaced branches, consumes exact-head Jankurai proof, and opens protected fast-forward PRs.
- `A6-ADP-007`: Storage repair, scrub, repack, rewrap, and control maintenance use signed WorkUnits
  through the same launch inventory.

## Distributed Shard and mounts

- `A6-STO-001`: `jain-shard` owns bounded chunking, compression, crypto, erasure, and local I/O;
  hub owns keys, indexes, manifests, refcounts, placement, reservations, tiers, receipts, and repair;
  native `jainnode` owns FUSE and node storage operations.
- `A6-STO-002`: There is no `jain-shardd` daemon or alternate storage execution path.
- `A6-STO-003`: Hierarchical manifests support resumable encrypted uploads, range reads, generation
  fencing, dedup, refcounts, tombstones, GC, repack, point repair, rewrap, tiering, and orphan repair.
- `A6-STO-004`: Publication follows distinct-domain staging, fsync, read-back reconstruction,
  plaintext verification, and synchronous hub/shadow journal commit; replay exposes only an old or
  new verified representation.
- `A6-STO-005`: The bounded QUIC ciphertext plane requires mTLS identity and one-purpose signed
  capability binding object, representation, byte ceiling, ControlPosition, expiry, and mutation
  nonce.
- `A6-STO-006`: Hub-signed node/rack/site topology governs placement. The first canary may use
  three-node full replication for Durable; Critical remains unavailable without a fresh independent
  offline/export target.
- `A6-STO-007`: Volume discovery never formats or claims. Claim/drain verifies canonical roots,
  filesystem UUID, ext4/XFS policy, `openat2 RESOLVE_BENEATH`, thresholds, and empty-drain receipts.
- `A6-STO-008`: Native `jainnode` uses Rust `fuser` without libfuse and binds mounts to actor,
  UID/GID, node, project, immutable snapshot, mode, staging bounds, expiry, and revocation epoch.
- `A6-STO-009`: Mounts reject traversal, links, devices, sockets, writable mmap, live Git stores,
  databases, shard internals, arbitrary roots, and stale grants; fence/revocation closes handles,
  purges plaintext, unmounts, and records cleanup.
- `A6-STO-010`: Default key custody uses encrypted systemd credentials, TPM sealing when available,
  an owner-custodied encrypted recovery copy, and short-lived zeroizing node grants.

## Interfaces, packaging, and canary

- `A6-REL-001`: Mutating storage APIs require actor context and Idempotency-Key; racing changes
  require If-Match; responses bind version, receipt, and ControlPosition.
- `A6-REL-002`: CLI covers governed storage/artifact operations, run logs/outputs, and node mounts;
  credentials and enrollment tickets never appear in argv.
- `A6-REL-003`: Reproducible hub and CPU scratch artifacts are at most 150 MiB; pinned GPU artifacts
  are at most 750 MiB; models and toolchains remain separately signed.
- `A6-REL-004`: Native nodes run on xbabe1-3 with fixed witness set and quorum two. Shadow selection
  is read-only and owner activation signs exact shadow, witness, topology, volume, and network input.
- `A6-REL-005`: Caddy remains independently pinned and canary activation uses one route-array CAS;
  neither hub nor node receives Docker socket access or DinD.
- `A6-REL-006`: Activation consumes unchanged signed digests only after fresh-install rehearsal,
  72-hour mixed soak, and 24-hour AtomicSoul canary.
- `A6-REL-007`: Two isolated builds reproduce identical digests and pass rootfs allowlists, SBOM,
  signatures, offline install, size limits, native/CPU equality, and deployment parity.
- `A6-REL-008`: Every touched repository ends on protected fast-forward history with exact-head
  proof, immutable tag, and authority binding.

## Acceptance and explicit deferral

- `A6-ACC-001`: Wire tests cover tamper, role, expiry, replay, revocation, stale epoch, and digest
  conflict while preserving V1 golden bytes.
- `A6-ACC-002`: Engine E2E covers checkpoint/retry, exact-model inference idempotency, reserved
  serving, provider outage, research cancellation, and governed interactive sessions.
- `A6-ACC-003`: Storage qualification covers stable FastCDC, insertion reuse, bounded 100 GiB
  streaming, dedup privacy, missing/corrupt shards, disk-full, rotation, crash, and orphan recovery.
- `A6-ACC-004`: Performance gates are explicit receipts: ingest throughput, visibility latency,
  containment latency, repair insertion, and cache bounds.
- `A6-ACC-005`: FUSE and shadow fault matrices prove containment, cleanup, zero acknowledged loss,
  zero split brain, protected-ref RPO zero, and owner-operated RTO at most 15 minutes.
- `A6-DEF-001`: Full three-lens SPA qualification is deferred and cannot be omitted or passed by
  compute/storage evidence.
- `A6-DEF-002`: Production JARVIS qualification is deferred.
- `A6-DEF-003`: Full prior-release migration is deferred.
- `A6-DEF-004`: Formal-GA and final two-owner release-index gates are deferred; this cut is a
  production-controlled canary only.
