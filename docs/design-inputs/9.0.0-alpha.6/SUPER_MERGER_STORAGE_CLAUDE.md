<!-- Root-canonical parallel specification. Identity = SHA-256 recorded in UPGRADE_CHAT.md
     (the split root is not a git repository). Companion documents:
     SUPER_MERGER_CLAUDE.md (parent; this document blows out its §6) and
     SUPER_MERGER_CODEX.md (reconciled decision-by-decision in Appendix R). -->

# SUPER MERGER STORAGE — Claude engineering specification (`jain-shard`)

This document is the full engineering specification for the SUPER MERGER distributed storage
fabric. It supersedes and blows out `SUPER_MERGER_CLAUDE.md` §6 ("Distributed storage —
`jain-shard`", ~40 lines) into a complete subsystem design, and it reconciles every decision
against `SUPER_MERGER_CODEX.md` §10.1–§10.4 (Appendix R). Where this document and a parent
document are read together, the narrower requirement governs; a genuine conflict is resolved by
reviewed amendment before any implementation lane opens.

**Specification only.** This document authorizes no source edit, branch, push, PR, merge,
deployment, migration, or service change. Every implementation lane requires its own
coordination claim in `UPGRADE_CHAT.md`, the owning repository's rules, the reviewed no-bypass
lifecycle, and exact-head proof. The 8.0.1 release candidate and all active release lanes are
unaffected by this document's existence.

## 0. Owner decisions (recorded 2026-07-17, binding on this specification)

- **OD-S1 — Inline dedup, with alerting.** The storage fabric performs inline, project-scoped,
  chunk-level deduplication before encryption (the Codex §10.2 model), AND surfaces duplication
  fast: per-project ratio counters updated on every write, threshold alerts, and a periodic
  duplicate-cluster report. This strikes the parent Claude spec's "no chunk-level dedup" v1
  non-goal.
- **OD-S2 — One engine, three faces, no second consensus.** One engine crate (`jain-shard`)
  is embedded in `jainnode` on full workers; a storage-only `jain-shardd` binary (static musl,
  `FROM scratch` container, single file) serves storage-class boxes and enrolls with the hub
  like a worker; the same binary runs a single-node standalone mode with local metadata for
  dev/edge/appliance use. Placement authority stays hub-owned in hub mode and node-local in
  standalone mode; there is no gossip, no DHT, and no second distributed consensus anywhere.
- **OD-S3 — Runaway-growth vigilance without privileges.** Near-instant growth detection is
  ingest-path rate tracking (everything written through the store) plus a statvfs mount
  sentinel for foreign writers. No fanotify, no inotify fleet, no added capabilities — the
  scratch container stays capability-free.
- **OD-S4 — Pure-Rust codecs in v1; zstd is a recorded lever.** The v1 codec lineup is 100%
  Rust (`lz4_flex` for ingest/warm, `brotli` for cold). The on-disk format is codec-agile
  (codec IDs in every chunk record and AAD), and codec id 4 is reserved for zstd: the owner may
  later enable a zstd encoder (C-FFI exception or a matured pure-Rust compressor) with zero
  format change. Default remains pure Rust; flipping the lever is an explicit reviewed owner
  decision, never a silent drift.

## 1. Scope, authority, and the reconciliation ledger

### 1.1 Scope

`jain-shard` stores every bulk byte the merged product owns: datasets, model bundles,
checkpoints, reports, logs and run outputs (as append streams), knowledge artifacts, build-cache
objects, git bulk objects/LFS, and repair evidence. It is one content-addressed, encrypted,
compressed, deduplicated, erasure-coded namespace with temperature tiers and built-in vigilance
(growth + duplication). It is the successor format to SCQ v2's artifact store (migration in
§11), the storage backend behind `jeryu-cache`'s receipt/poisoning front end, and the byte
plane beneath the one CAS of `SUPER_MERGER_CLAUDE.md` §1.4.

Never on the layer (unchanged from the parent): RedlineDB and live git bare stores (the shard
map lives in RedlineDB — circular; they replicate via the shadow master and only their periodic
snapshots/bundles archive into the shard layer), workcell checkouts, warm pools, node scratch,
PTY buffers.

### 1.2 Authority

- Hub mode: manifests, placement, KEK wraps, and receipts live in RedlineDB; placement is
  hub-owned weighted rendezvous hashing; repair/scrub/tiering/dedup-report jobs are WorkUnits
  in the one scheduler.
- Standalone mode: the identical schemas live in a local redb database; the identical state
  machines run on an embedded single-node executor. The on-disk chunk/shard format is byte-
  identical in both modes, so a standalone store is adoptable by a hub later via manifest
  import (§10.4).

### 1.3 Reconciliation ledger — what this document amends

The two parent specifications disagreed on six storage mechanics. This ledger resolves all six,
and lists every other amendment this document makes. Appendix R carries the full
decision-by-decision map against Codex §10.

| # | Topic | Parent positions | Resolution here |
|---|---|---|---|
| L1 | Chunking | Codex: content-defined, avg 4 MiB. Claude §6.2: fixed 8 MiB ciphertext chunks | **FastCDC over plaintext, min/target/max = 1/4/16 MiB** (§2.2). OD-S1 dedup requires plaintext CDC; fixed ciphertext chunking cannot dedup at all |
| L2 | Pipeline order | Codex: chunk → dedup → compress → encrypt per chunk. Claude §6.2: compress → encrypt stream → split ciphertext | **Codex order adopted** (§3.1). Claude §6.5's "no chunk-level dedup (ciphertext defeats it)" is struck — it was an artifact of the wrong pipeline order |
| L3 | Key model | Codex: per-chunk random DEK wrapped by versioned project KEK. Claude §6.2: per-artifact key wrapped by hub master key | **Codex model adopted + one addition**: a per-project `dedup_key` wrapped by the Root Key directly, so fingerprints survive KEK rotation (§4.2) |
| L4 | Dedup fingerprint | Codex: HMAC-SHA-256(project-dedup-key, uncompressed chunk) | **Amended to keyed BLAKE3** (§3.2): same keyed-PRF security claim, ~5–10× faster, pure Rust; the fingerprint algorithm ID rides in the AAD and index so a future PRF swap is a new fingerprint domain, not a format break |
| L5 | Erasure derivation | Codex: RS 4+f, else f+1 replicas. Claude §6.3: m=f, k=min(6, live−f), degrade to (m+1)-replication | **Claude's adaptive-k adopted** (§6.2): Codex's 4+f is a single point on the same curve; adaptive k up to 6 buys 1.33× overhead for `durable` on ten nodes. All Codex §10.3 honest-reporting duties kept verbatim |
| L6 | Profile names | Codex: scratch/balanced/protected/critical. Claude: scratch/standard/durable/critical | **Claude names win; Codex names become accepted aliases** (`balanced`→`standard`, `protected`→`durable`) — one match arm, zero migration (§6.1) |
| L7 | Compression | Both parents: "zstd level 3", fixed | **Codec-agile matrix, pure-Rust v1 lineup** (§5, OD-S4). Both parents said the same wrong-for-this-constraint thing |
| L8 | Bulk-rewrite rule | Codex line 945: bulk ciphertext is never rewritten except algorithm migration | **Kept, with tier migration classified as a declared algorithm-migration-class rewrite** (§7.5): scheduled, receipt-producing, atomic-publish. Key rotation alone still never rewrites bulk |
| L9 | Tiering | Absent from both parents (durability tiers only) | **New §7**: hot/warm/cold temperature system with an in-memory plaintext hot cache, generalizing the parent's model-bundle "pin cache, not a copy outside the system" |
| L10 | Growth vigilance | Absent from both parents (soak-level thresholds only) | **New §8**: inline dual-EWMA ingest detection + mount sentinel + self-protection ladder |
| L11 | GC enforcement | SCQ v2 records refcounts/retention but nothing ever deletes | **New §9**: enforced refcounts, manifest-driven deletion, quarantine window, receipt-producing sweeper |
| L12 | Object/job size caps | SCQ v2: 512 MiB object / 1 GiB job ceilings | **Removed** via two-level streamed manifests + bounded transfer windows (§2.3); memory is bounded independent of object size |

## 2. Object model and addressing

### 2.1 Identifiers

- **Object id** = SHA-256 over the whole plaintext object, unchanged from SCQ v2
  (`ArtifactId::from_digest`). It is computed once per object, off the per-byte hot path, and
  is baked into the SCQ wire protocol, RedlineDB rows, R&D checkpoint digests, and git SHA-256
  interop — changing it buys nothing and costs a family-wide migration. `ManifestRoot` carries
  `id_scheme: u8` (0 = SHA-256) so a future BLAKE3 object-id era changes manifests only; chunks
  are fingerprint-addressed and never need rewriting.
- **Chunk fingerprint** = keyed BLAKE3 (32 bytes) under the per-project `dedup_key`, over the
  uncompressed chunk (§3.2). All per-chunk and per-shard hashing is BLAKE3.
- **Shard checksum** = unkeyed BLAKE3 over the shard's ciphertext bytes, verified on receipt,
  on read, and by scrub.

### 2.2 Chunking

FastCDC (2020 variant, 64-bit gear hash, normalization level 2) over plaintext with
**min 1 MiB / target 4 MiB / max 16 MiB** (frozen constants, §18). Rationale: content-defined
boundaries give shift-resistant dedup (an insertion re-aligns within one chunk instead of
cascading); min = target/4 bounds manifest bloat, max = 4×target bounds worst-case per-chunk
memory at 16 MiB. Throughput >2 GiB/s single-core — chunking is never the ingest bottleneck.
Crate: `fastcdc` (MIT, pure Rust).

### 2.3 Manifests — two-level, streamed, uncapped

```
ManifestRoot {
  object_id: [u8;32], id_scheme: u8, total_len: u64,
  chunk_count: u64, segment_count: u32,
  segments_root: [u8;32],          // BLAKE3 merkle over segment digests
  profile: DurabilityProfile, project_id: [u8;16],
  data_class: DataClass, policy: PolicyRef,
  tier: Tier, tier_changed_at: u64, // §7
  created_at: u64, generation: u64, // §2.4
}
ManifestSegment {                   // ≤ 4096 entries per segment
  entries: [{ fingerprint: [u8;32], plain_len: u32, cum_plain_off: u64 }],
}
```

Encryption/codec metadata deliberately lives in the chunk index, not the manifest — chunks are
shared across objects; manifests only order them. A 1 TiB object is ~262k chunks = 64 segments
of ~180 KiB; a range read touches the root plus O(1) segments. The SCQ v2 512 MiB object and
1 GiB job ceilings are deleted; the per-stream in-flight budget (§3.5) bounds memory
independently of object size.

**Integrity ladder** (range reads verify without whole-object rehash):
AEAD tag (ciphertext + AAD) → per-chunk `plain_hash` (BLAKE3, post-decompress) → segment
digest → `segments_root` merkle → whole-object SHA-256 verified once at commit and thereafter
vouched by the recorded, authenticated manifest.

### 2.4 Append streams — the log model

A "growing file" in a content-addressed store is a **stream**: a chain of immutable
generations. Each `flush()` seals the current CDC buffer (the final sub-minimum chunk is sealed
as-is — determinism beats the marginal dedup loss at flush boundaries), appends entries to the
open segment, and publishes `ManifestRoot{generation: n+1}` superseding generation n. Both
generations are recorded; readers pin a generation; `tail --follow` re-resolves the head
generation. The ingest path exports per-stream counters — `bytes_in`, `chunks_out`,
`dedup_hit_bytes`, `flush_rate` — which are exactly the signal the vigilance system (§8)
consumes. Streams default to the `scratch` or `standard` profile and are first-class citizens
of the runaway-growth detector, including its auto-throttle policy (§8.1).

## 3. The write path

### 3.1 Pipeline

```
client bytes
  → FastCDC (1/4/16 MiB, plaintext)                      §2.2
  → keyed-BLAKE3 fingerprint (project dedup_key)          §3.2
  → dedup lookup (bloom → 2Q cache → chunk_index)         §3.3
      hit  → refcount++ (resurrect from quarantine if needed), record in manifest, done
      miss → compress (temperature-class codec + gate)    §3.4, §5
           → encrypt (fresh DEK, XChaCha20-Poly1305, AAD) §4
           → stripe assembly → erasure → placement        §6
           → chunk_index insert + refcount=1 (one txn)
  → manifest segment append; commit txn publishes ManifestRoot
```

Dedup, compression, and encryption are per-chunk and embarrassingly parallel; the engine
pipelines chunks across a bounded worker pool sized to available cores, so "world-class …
without slowing us down" is an architecture property: the slow work happens per-chunk on
independent CPUs while the stream keeps flowing.

### 3.2 Fingerprinting

`fingerprint = BLAKE3::keyed(project.dedup_key, uncompressed_chunk)`. Keyed BLAKE3 is a PRF
with the same security claim as HMAC-SHA-256 for this use (the key is project-secret, so no
offline confirmation attack exists without it) at ~5–10× the throughput, pure Rust with runtime
SIMD dispatch. A dedup hit is fingerprint equality plus a `plain_len` equality sanity check; no
byte compare (keyed 2⁻²⁵⁶ collision probability is below hardware error rates). Fingerprints
never leave the trusted storage service (§8.4's equality-oracle rules). `fingerprint_alg: u8`
rides in the chunk index and AAD; a future PRF swap opens a new fingerprint domain rather than
breaking the format. Amends Codex §10.2 (ledger L4).

### 3.3 Dedup index — three layers, crash-consistent

- **Layer 1 — split-block bloom filter**, 12 bits/key (~0.3% false positives): 99.7% of
  unique-chunk misses never touch the database. Persisted as snapshot + insert counter,
  replayed on startup; full rebuild by table scan is the fallback (minutes at 100 M keys).
- **Layer 2 — sharded 2Q fingerprint cache**, 1 M hot entries in-crate (~80 MB). No external
  cache crate.
- **Layer 3 — `chunk_index`** (redb/RedlineDB, §10): fingerprint → full chunk record
  (~200 B including the wrapped DEK).

Footprint at 100 M chunks (≈ 400 TiB logical at 4 MiB average): ~30 GB on disk, **< 256 MB
RAM** (150 MB bloom + 80 MB 2Q). Crash consistency ordering: (1) write + fsync ciphertext
shard/chunk files; (2) one metadata transaction inserting the chunk record and incrementing the
refcount; (3) manifest commit transaction. A crash between 1 and 2 leaves an orphan file with
no index entry — reaped by the sweeper after the quarantine window (§9.3). Ingest is idempotent
by fingerprint; replays are no-ops.

### 3.4 Incompressibility gate

Compress the first 64 KiB with the temperature-class codec; if the probe ratio exceeds 0.97,
store `codec_id = 0` (raw). Unconditionally: if final `comp_len > plain_len − 512`, discard the
compressed form and store raw — compression can never inflate stored bytes. A magic-byte fast
path (zstd/gzip/xz/jpeg/png/mp4/zip frames, parquet-snappy) skips the probe at ingest but sets
`retry_cold: true` so the cold pass may attempt one aggressive re-try (containers sometimes
still yield 5–15%).

### 3.5 Bounded memory

Per stream: ≤ 4 chunks in flight × 16 MiB max + one 64 MiB stripe buffer → a **hard 128 MiB
ceiling per stream regardless of object size**, enforced by a semaphore. Transfer windows are
bounded and resumable. This satisfies Codex's "memory use MUST remain bounded independently of
object size" and is what licenses deleting the old object-size caps (ledger L12).

## 4. Cryptography

### 4.1 Cipher and nonces

**XChaCha20-Poly1305 everywhere** — already the family cipher (`chacha20poly1305` 0.10.1 in the
lockfiles), constant-time in pure Rust, and the 192-bit nonce space makes random-nonce-per-
encryption-event safe to ~2⁸⁰ events: no counters or nonce-derivation schemes to get wrong.
Every encryption event draws a fresh 24-byte nonce from OsRng. Recompression re-encrypts with a
**fresh DEK and fresh nonce** (the fingerprint and the plaintext are invariants of the chunk;
the ciphertext is not). AES-256-GCM was considered and rejected: hardware AES is faster on
AES-NI cores, but the family already ships, audits, and trusts one cipher, and ingest is
pipeline-parallel — the cipher is not the bottleneck.

### 4.2 Key hierarchy

```
Root Key (RK)   file format JSHRK1, mode 0600:
  magic(7) ‖ version u16 ‖ alg u8 ‖ params ‖ material
  alg 0 = raw 32B · 1 = argon2id passphrase-wrapped · 2 = reserved (TPM/KMS)
  │
  ├── wraps → Project KEK v1..vN   (32B random, versioned; stored in RedlineDB/redb)
  │             rotation = mint vN+1 + background DEK rewrap; bulk ciphertext untouched
  │             cryptographic erasure = destroy every live wrap of every vN
  │             │
  │             └── wraps → per-chunk DEK  (32B random, unique per unique chunk;
  │                          stored as 72B blob: nonce(24) ‖ ct(32) ‖ tag(16);
  │                          encrypts the compressed chunk under XChaCha20-Poly1305)
  │
  └── wraps → Project dedup_key    (32B random; the keyed-BLAKE3 fingerprint PRF key;
                deliberately wrapped by the RK, not the KEK, so fingerprints survive
                KEK rotation; rotates only on full project re-ingest — documented)
```

Per-chunk DEKs are what make OD-S1 sound: a chunk shared by two artifacts has one key of its
own, rotation is a metadata rewrap (walk the index, rewrap 72-byte blobs), and cryptographic
erasure is wrap destruction plus a synchronous hot-cache project purge (§7.3) **before** the
erasure receipt issues. This adopts the Codex model (ledger L3) and fixes the verified SCQ v2
weakness — a bare, structureless 32-byte key file (`artifact_store/support.rs`,
`load_or_create_key`) that encrypted everything directly.

All plaintext key material lives in `Zeroizing` buffers (`zeroize` crate); the RK file is
created `create_new` with 0600 and refuses group/other-readable permissions on open (the one
SCQ behavior worth keeping).

### 4.3 AAD structures

Chunk AAD `JSHAAD1` (fixed layout, versioned):
`magic(7) ‖ format_version u8 ‖ storage_domain_id[16] ‖ project_id[16] ‖ fingerprint[32] ‖
fingerprint_alg u8 ‖ codec_id u8 ‖ codec_params u8 ‖ cipher_id u8 ‖ plain_len u64 BE ‖
comp_len u64 BE`.

KEK-wrap AAD `JSHKW1` binds domain ‖ project ‖ kek_version ‖ fingerprint; RK-wrap AAD `JSHKEK1`
binds domain ‖ project ‖ kek_version.

Binding both lengths means the reader has its decompression output size (`plain_len`) and input
size (`comp_len`) **authenticated before decompressing** — the decompression-bomb guard is
cryptographic, not advisory. Chunk order is deliberately not in chunk AAD (chunks are shared);
order is bound by the authenticated manifest ladder (§2.3), exactly per Codex §10.2.

## 5. Codec matrix and codec agility

Every chunk record and its AAD carry `codec_id: u8` + `codec_params: u8`. Codecs can be added
or retired without a format break; the cold-tier migration machinery (§7.5) re-encodes old
chunks automatically when a codec is retired.

| codec_id | Codec | Crate (license) | Params | Role |
|---|---|---|---|---|
| 0 | none | — | — | incompressible / gated (§3.4) |
| 1 | lz4 | `lz4_flex` (MIT, pure Rust) | block, fast | **ingest + warm default**: >2 GiB/s encode, >3 GiB/s decode — compression that is literally not felt |
| 2 | deflate | `miniz_oxide` via flate2 (MIT OR Zlib OR Apache-2.0) | — | decode-compat only (family already ships flate2) |
| 3 | brotli | `brotli` (BSD-3-Clause/MIT dual; two BSD-3-only transitive deps — §14) | q, lgwin | **cold**: q10–q11 / lgwin 24 beats zstd-19 on text-like data; single-to-double-digit MiB/s is irrelevant because cold recompression is background-only |
| 4 | zstd | decode: `ruzstd` (MIT, pure Rust); **encode: reserved** | level, dict_id | the OD-S4 lever |

Honest notes, recorded so nobody re-litigates them from vibes:

- `ruzstd`'s decoder is mature and fuzz-hardened; its compressor implements only a basic
  fast-level match finder — no long-distance matching, no dictionary training, ratios well
  below libzstd level 3. It is a compatibility decoder, not a cold-tier compressor, and this
  specification does not pretend otherwise.
- Per-project **trained** dictionaries (zdict-quality) require libzstd's trainer and are gated
  behind the OD-S4 lever. Pure-Rust v1 ships without trained dictionaries; the cold tier leans
  on brotli's built-in static dictionary, 16 MiB max chunks (a big intra-chunk window), and the
  fact that cross-file redundancy is already dedup's job (OD-S1). If the owner flips the lever,
  dictionaries arrive as small full-replica `critical` objects, refcounted and versioned via
  `dict_id` in `codec_params`, trained per project only (a dictionary is trained on plaintext
  and must never cross a project boundary).
- Tier moves never perturb the dedup map in either direction: chunk identity is the keyed hash
  of the **uncompressed** chunk.

## 6. Durability profiles and erasure coding

### 6.1 The risk knob — a profile, never raw k+m

| Profile (Codex alias) | Survives | Derivation | Fallback when live nodes are short | Small object (<2 MiB ct) |
|---|---|---|---|---|
| `scratch` | 0 nodes | local only, TTL-bound, no stripes | — | 1 copy |
| `standard` (`balanced`) — default | 1 node | m=1, k=min(6, live−1) | live<3 → 2× replication | 2 replicas |
| `durable` (`protected`) | 2 nodes | m=2, k=min(6, live−2) | live<4 → 3× replication | 3 replicas |
| `critical` | 3 nodes | m=3, k=min(4, live−3); requires a configured, signed offline/export target within its RPO | live<5 → 4× replication | 4 replicas |

Adaptive k (ledger L5): on a ten-node cluster `durable` costs ~1.33× instead of 3× replication;
the k≤4 cap on `critical` bounds stripe width, repair fan-in, and reconstruction latency where
they matter most. When topology cannot satisfy the profile, the store **degrades honestly to
(f+1)-way replication and says so** — every project storage surface continuously displays
requested vs achieved node/rack/site durability, degradation reason, repair backlog, and
offline-target freshness; the system never silently claims a level the topology cannot satisfy.
Failure-domain (rack/site) labels require administrator attestation — a worker's self-report
cannot establish durability. A `critical` policy counts as achieved only while the signed
offline/export target is complete and no older than its configured RPO. (All Codex §10.3
duties, kept verbatim.)

Data classes can force floors (release artifacts ≥ `durable`; git refs, membership, audit logs,
control snapshots, trusted shared knowledge have a `durable` floor when topology permits, with
a persistent risk AttentionItem when it cannot).

### 6.2 Erasure mechanics

Erasure is over **ciphertext** — nodes store ciphertext-only shards, so a stolen or
decommissioned disk leaks nothing. The stripe assembler packs sealed chunks into ~64 MiB
stripes, splits each stripe into k data shards + m parity via `reed-solomon-simd` (MIT, pure
Rust, GF(2¹⁶), `std::arch` SIMD); shards are padded to 64-byte multiples for the SIMD kernels.
Each shard carries an unkeyed BLAKE3 checksum verified on receipt, on read, and by scrub.
Objects whose total ciphertext is under 2 MiB take full replicas instead of stripes (frozen
cutoff, §18) — parity math on sub-stripe objects wastes CPU and multiplies read fan-out, and
this quietly delivers what the parent's deferred "pack small knowledge artifacts" optimization
wanted.

### 6.3 Placement

- **Hub mode**: hub-owned weighted rendezvous hashing (HRW over enrolled node ids, weighted by
  free shard budget from heartbeats), recorded in RedlineDB. The scheduler reads the same
  records for input-locality hints. Reads are hub-gateway in v1 (reconstruct from any k,
  stream); peer reads via short-lived signed chunk capabilities are v1.1, unchanged from the
  parent.
- **Standalone mode**: each configured backing mount is a failure domain. With M ≥ 2 mounts,
  `standard` places m=1 across mounts; with one mount, profiles degrade to single-copy +
  integrity and the status surface says exactly that (the honest-reporting rule applies to
  mounts precisely as it applies to nodes). Free-space-weighted local HRW picks mounts.

## 7. The temperature system — hot, warm, cold

Temperature is orthogonal to durability: durability says how many failures bytes survive;
temperature says how fast they come back and how hard they are squeezed at rest.

### 7.1 Model

- Tier state (`hot|warm|cold`) is a **per-object (per-manifest) property**; objects are the
  unit of user intent. Physical migration executes per-chunk with a **refcount gate**: a chunk
  re-encodes to cold only when every manifest referencing it is itself cold (checked at
  WorkUnit execution time, not scan time). A chunk shared by a hot dataset and a dead log stays
  fast — per-chunk temperature would be ill-defined under dedup.
- **hot** is a residency state, not a stored attribute: the plaintext chunk sits in the
  in-memory cache.
- **warm** is the default at-rest state: the ingest format (lz4), on disk. Every object is
  born warm.
- **cold**: untouched ≥ `tiers.cold_after_days = 7` (min 1 — the user's "days", with margin
  for weekend idleness) → re-encoded brotli q10/lgwin24 by the migration WorkUnit. Objects
  < 64 KiB never demote (receipt + metadata overhead exceeds savings; they are full replicas
  anyway).

### 7.2 The access clock — temperature without write amplification

Reads update an in-memory dirty map `(project_id, object_id) → { last_access_hbucket: u32,
freq: u8 }` (hour-bucket granularity; freq halves daily), bounded at 4,096 entries (~256 KiB).
It flushes every 60 s, or when full, as one batch — and only entries whose **hour bucket
changed** are written: worst case one metadata write per object per hour regardless of read
volume. A companion index `access_by_bucket (hbucket, object_id)` is maintained in the same
batch, giving demotion scans an oldest-first range read that is O(candidates) — there is never
a full namespace sweep. Crash loss of ≤ 60 s of clock deltas is benign: an object merely looks
colder than it is, and a cold read repromotes it.

**System reads never touch the clock or the cache.** Scrub, repair, tier migration, and dedup
reports bypass access accounting entirely — otherwise weekly scrub would keep the whole store
warm forever.

### 7.3 The hot tier — in-memory plaintext chunk cache

- **Algorithm**: byte-weighted **S3-FIFO**, hand-rolled in-crate (~400 lines): a probationary
  FIFO at 10% of budget, a main FIFO at 90%, and a ghost queue of recently evicted keys (keys
  only, 2× main's entry count). An entry promotes probation→main on its second hit; one-hit
  wonders exit through probation without disturbing the working set. S3-FIFO gives
  TinyLFU-class hit ratios with no frequency sketch and trivially auditable code, and — the
  binding requirement — scan resistance: a large streaming read cannot thrash the working set.
  `moka` is rejected on dependency weight (crossbeam tree); `quick_cache` (MIT) is named as the
  sanctioned fallback if the hand-roll underperforms its drill (§15, M5c). The hand-roll wins
  because four needed hooks are awkward in off-the-shelf caches: byte-weighted eviction, a
  single mlock-able slab arena, zeroize-on-evict, and project-scoped purge.
- **Contents & keying**: decompressed plaintext chunks keyed `(project_id, fingerprint)` — a
  hit skips disk, decrypt, and decompress. Range reads admit whole chunks, so repeated ranged
  access to the same region hits.
- **Admission**: sequential streams over `hot_cache.stream_bypass_bytes = 256 MiB` bypass
  admission but record in the ghost queue (a second pass admits); no single chunk larger than
  1/8 of budget is admitted. No negative caching — a "known absent" cache is an
  equality-oracle surface, and the miss path is a single local B-tree point lookup.
- **Sizing**: a capacity, never a preallocation — the arena grows only under use, so the
  family idle-RSS gates (hub < 1 GiB, node < 200 MiB) hold by construction. Defaults:
  embedded/hub-gateway `clamp(5% of MemTotal, 64 MiB, 2 GiB)`; standalone `jain-shardd`
  (a box dedicated to storage) `clamp(25% of MemTotal, 64 MiB, 8 GiB)`. Config
  `cache.hot_bytes = "auto" | <bytes>`.
- **Trust boundary**: the plaintext cache exists **only** in the hub gateway process and in
  standalone `jain-shardd`. Hub-attached storage nodes remain ciphertext-only — their only
  caching is the OS page cache over shard files. Arena pages get `madvise(MADV_DONTDUMP)`
  always; `mlock` per `cache.mlock = try|require|off` (default `try`: degrade with a logged
  warning if RLIMIT_MEMLOCK is short). Entries are zeroized on eviction. **Project
  cryptographic erasure and key rotation call `purge_project(project_id)` synchronously before
  the erasure receipt issues** — cached plaintext is inside the trust boundary and dies with
  the keys. This generalizes the parent's model-bundle rule: a pin cache, not a copy outside
  the system.
- **Pinning**: model bundles and other lease-scoped pins live in a separate pin set with its
  own budget (`cache.pin_max_bytes`, default 0), outside eviction.
- **No warm-up persistence**: the cache never persists across restart — plaintext must never
  touch disk. Cold start is honest.
- **Effectiveness**: per-shard hit/miss/bypass counters aggregate to hit ratio and
  bytes-served-from-RAM in `jain-shardd status` / `cache stats` (internal counters; the family
  has no metrics endpoint).
- **Concurrency**: `min(next_pow2(2×cores), 16)` shards, each a mutex-guarded S3-FIFO; no lock
  held across decrypt/decompress.

### 7.4 The warm tier

Warm is the ingest format — no re-encoding. Sequential manifest traversal speculatively
decodes the next `readahead.chunks = 2` chunks into the hot cache under a per-stream prefetch
budget of 32 MiB, and issues `posix_fadvise(POSIX_FADV_WILLNEED)` on local shard files.
Prefetched-but-never-read chunks count as bypass traffic (probation only) so prefetch can never
evict the working set.

### 7.5 The cold tier — aggressive squeeze, honest mechanics

- **Demotion scan**: `TierScanV1` WorkUnit every `tiers.scan_interval_hours = 6` range-reads
  `access_by_bucket` from the oldest bucket up to `now − cold_after_days`, applies the §7.1
  filters, and emits `RecompressV1{manifest_refs, direction: Cold}` WorkUnits batched at
  ≤ 8 GiB logical each.
- **The migration itself** runs inside the trusted storage service: read old chunks → verify →
  decompress → recompress brotli `tiers.cold_quality = 10` (range 9–11), lgwin 24 → encrypt
  with **fresh DEK + fresh nonce** → write into a temporary namespace → validate the
  end-to-end object hash against the manifest → atomically publish the new manifest revision
  (chunk entries now carry codec 3) → decrement refcounts on the old shards → emit a signed
  **TierMigrationReceipt** into the manifest's migration history. This is byte-for-byte the
  repair choreography.
- **Amendment box (ledger L8).** Codex §10.2 states bulk ciphertext is never rewritten except
  for algorithm migration. This specification classifies scheduled tier migration as a
  **declared, receipt-producing, algorithm-migration-class rewrite**. The key-rotation
  guarantee is unchanged: rotation alone still rewraps DEKs without touching bulk ciphertext.
- **Promotion**: a cold read serves immediately (brotli decode is fast; cold's cost is
  compression CPU at demotion time, not read latency), populates the hot cache, and bumps the
  clock. Rewriting back to warm (`RecompressV1{direction: Warm}`) fires only on **sustained**
  re-access: ≥ 3 accesses in distinct hour buckets within 72 h — a script re-reading a file
  500 times in one minute does not qualify; the hot cache absorbs it.
- **Throttles**: per-node migration cap `tiers.max_migrate_mibps = 40`; all tiering pauses
  when disk free < 20%, PSI io some avg10 > 20%, or the repair backlog exceeds 100 pending
  units (repair always outranks tiering); a batch starts only with ≥ 2× its size in free space
  (temp-namespace headroom). In standalone mode the identical state machine runs on the
  embedded executor and self-throttles: if foreground read p99 exceeds 2× its rolling
  baseline, migration pauses 30 s.

### 7.6 New WorkUnit kinds

`TierScanV1 | RecompressV1 | DedupReportV1 | GcSweepV1 | MigrateArtifactV1` join
`RepairV1 | ScrubV1` in the one scheduler, all Opportunistic tier (preemptible under the
locked preemption law), never blocking ingest. Repair and scrub keep their existing contracts.

### 7.7 Temperature matrix

| Tier | Resides | Codec | 4 MiB-chunk latency target | Entered by | Left by |
|---|---|---|---|---|---|
| hot | RAM, plaintext, mlock'd | none (decoded) | < 1 ms | S3-FIFO admission on authorized read | eviction, purge, restart |
| warm | disk, ciphertext | lz4 | ≤ 20 ms local / ≤ 60 ms hub-gateway | ingest (default); sustained cold re-access | untouched ≥ 7 d ∧ ≥ 64 KiB → RecompressV1(Cold) |
| cold | disk, ciphertext | brotli q10/lgwin24 | ≤ 30 ms local | RecompressV1(Cold) + receipt | RecompressV1(Warm) on sustained re-access |

## 8. Vigilance — runaway growth and duplication

### 8.1 Ingest-path growth detection — instant by construction

Per-stream and per-project byte rates are tracked by **dual EWMAs** — fast half-life 10 s, slow
half-life 600 s (the same EWMA idiom the fabric scheduler already uses for rtt/bandwidth) —
updated per transfer window at a cost of tens of nanoseconds, **checked inline before the
window ack**. Trip condition:

```
fast > max(8 × slow, 8 MiB/s)  AND  cumulative stream bytes > 256 MiB
```

"Nearly instant" is literal: detection occurs on the very transfer window whose commit crosses
the threshold — latency is bounded by one window, i.e. milliseconds. Hysteresis: the trip
clears when `fast < 2 × slow` sustained for 60 s.

On trip: a signal row → derived `AttentionItem{ severity: warn (high if disk free < 20%),
kind: capacity, project_id, verb: inspect-stream }`, with `cap-stream` offered as the follow-up
action. Auto-throttle policy `growth.auto_throttle = scratch-only|off|all` (default
`scratch-only`): scratch-profile streams are auto-clamped by a token bucket (64 MiB burst,
refill = 2 × slow EWMA) and hard-capped at `scratch.max_stream_bytes = 32 GiB`;
`standard`/`durable`/`critical` streams are **never** auto-capped absent explicit owner policy.

A complementary **object-lineage detector** on the manifest-update path compares logical sizes
across generations: an object that doubles in under 1 h across ≥ 3 generations raises
`AttentionItem{capacity, warn, verb: inspect-object}` — this catches the re-uploaded runaway
log that never trips a single-stream rate.

### 8.2 The mount sentinel — foreign writers, no privileges

Every 5 s (per mount) the daemon reads `statvfs` — one syscall — and computes **unaccounted
consumption**: the mount's free-space delta minus its own accounted ingest, migration, and temp
writes. Anomaly: unaccounted rate > 5 MiB/s for 3 consecutive intervals (15 s). The daemon
continuously maintains a shallow dir-size cache of its own tree (`shards/`, `tmp/`, `dicts/`,
2 levels, a few KiB, updated from its own write accounting — never by walking). On anomaly it
runs a **bounded targeted scan**: readdir + statx at the mount's top level only; descend only
into entries whose delta against the cache explains > 50% of the anomaly; max depth 4; max
10,000 stat calls per scan. Result: `AttentionItem{node, high, verb: inspect-path}` naming the
offending path and its growth rate; if the offender is inside the daemon's own tree (a leaked
temp — an accounting bug), the verb becomes `gc-temp`. Inode-rate anomalies (already in the
heartbeat) ride the same machinery. This finds a foreign runaway log in seconds with no
CAP_SYS_ADMIN, no fanotify, and a hard cost ceiling (OD-S3).

### 8.3 Self-protection — the disk-pressure ladder

| Mount free | Action | AttentionItem |
|---|---|---|
| < 20% | pause all Opportunistic storage work (tiering, scrub, dedup reports) | — (telemetry) |
| < 10% | ingest backpressure: project token buckets clamp to 25% of recent rates | capacity / high / `expand-storage` |
| < max(5%, 2 GiB) | refuse new writes (typed error, CLI exit 5); reads, deletes, repair continue | capacity / critical / `free-space` |
| always | a 2% **repair reserve** stays writable by RepairV1 — repair can never deadlock against the refuse floor | — |

These are the same numbers the node heartbeat reports (`disk_available_bytes`), so hub
placement weights and local self-protection agree by construction.

### 8.4 Duplication vigilance

- **Inline counters, O(1) per write**: the dedup lookup that already happens per chunk bumps
  per-project `logical_bytes`, `unique_physical_bytes`, `dup_hit_bytes` (with 15-min and 1-h
  ring buffers), flushed in the same 60 s batch as the access clock. Duplication ratio =
  1 − unique/logical; **reclaimable-duplicate bytes** = Σ (refcount−1) × comp_len, maintained
  incrementally. A Misra-Gries top-64 sketch tracks the largest duplicate clusters.
- **Alert on the crossing write**: ratio > 30% AND dup_hit_bytes > 10 GiB →
  `AttentionItem{capacity, warn, project_id, verb: review-duplicates}`. The signal row is
  written by the write that crosses the threshold; the AttentionItem materializes on the next
  derivation pass (AttentionItems are derived, never stored).
- **`DedupReportV1`** (weekly, Opportunistic): builds 128-permutation MinHash sketches over
  each object's fingerprint set (in-crate, no dependency) and reports the top 20 object
  clusters with ≥ 90% chunk overlap and their logical-redundancy bytes — this names
  *user-level* duplication (copied datasets, forked checkpoints) that chunk counters cannot.
- **Equality-oracle rules, kept verbatim from Codex §10.2**: deduplication happens only inside
  the trusted storage service after authorization; APIs, receipts, quotas, and billing expose
  logical bytes and never expose chunk IDs, physical placement, or hit/miss state to a
  narrower ResourceGrant; ingest behavior is padded or asynchronously normalized where the
  threat model shows an equality oracle; a project needing isolation between resource cohorts
  uses separate storage domains and keys rather than project-wide dedup. Duplication reports
  are project-scoped (project-admin grants); the deployment admin sees aggregate ratios with
  no chunk identities; top-cluster rows carry opaque handles — a full-project grant may
  resolve a handle to example referencing objects. Cross-project duplication is invisible **by
  construction** (per-project dedup domains) and this specification says so rather than
  pretending to detect it.

### 8.5 Alert catalog

AttentionItem kinds are restricted to the existing enum (`capacity`, `node`); every alert
carries an actionable verb — a warning with no verb is telemetry, not an AttentionItem.

| Signal | Threshold (defaults) | kind | verb | severity |
|---|---|---|---|---|
| Ingest acceleration | fast ≥ 8× slow ∧ ≥ 8 MiB/s ∧ stream ≥ 256 MiB | capacity | `inspect-stream` (`cap-stream` action) | warn (high if disk < 20%) |
| Object runaway growth | size doubles < 1 h over ≥ 3 generations | capacity | `inspect-object` | warn |
| Foreign mount writer | unaccounted ≥ 5 MiB/s × 15 s | node | `inspect-path` / `gc-temp` | high |
| Disk pressure | free < 10% | capacity | `expand-storage` | high |
| Write refusal | free < max(5%, 2 GiB) | capacity | `free-space` | critical |
| Duplication | ratio > 30% ∧ dup bytes > 10 GiB | capacity | `review-duplicates` | warn |
| Tiering backlog | cold candidates unprocessed > 7 d | capacity | `review-tiering` | info |
| Durability shortfall | achieved < requested (any profile) | capacity | `review-durability` | high |

Dictionary-ratio regression (when the OD-S4 lever is on) deliberately has no AttentionItem —
it silently retrains.

## 9. GC, deletion, quarantine, receipts

SCQ v2 records `refcount` and `retain_until` and **nothing ever reads them to delete** — the
store is accidentally immortal. This section is the enforcement that was missing.

- **Refcounts enforced in-transaction**: every manifest commit increments its chunks; every
  manifest deletion streams its segments decrementing them.
- **Deletion is manifest-driven**: authorization + retention check → manifest tombstone →
  refcount decrements → fingerprints reaching zero move to `quarantine` with
  `quarantine_until = now + 72 h` (scratch: 1 h).
- **The sweeper runs**: `GcSweepV1` WorkUnit (hub) / embedded task (standalone) deletes
  ciphertext past the window and emits a signed **GC receipt**
  `{project, manifest_ids, chunks_deleted, bytes_reclaimed, sweeper_identity, ts}` into
  RedlineDB/redb — receipts + quarantine are the jeryu-cache precedent applied to the shard
  layer.
- **Resurrection rule**: a write hitting a quarantined fingerprint un-quarantines it
  (refcount 0→1) instead of re-storing — dedup and GC compose instead of racing.
- **Orphan reaper**: scrub reconciles on-disk files against the index; unindexed files older
  than the quarantine window are quarantined (this closes the §3.3 crash window).
- Repair keeps the Codex contract: system-priority Run, signed placement plan, fence token,
  reconstruct into a temporary namespace, validate the end-to-end hash, atomically publish,
  only then retire obsolete shards. Deletion of shared chunks happens only when no live
  project-scoped manifest references them; key rotation and cryptographic erasure require
  explicit runbooks and recovery proof.

## 10. Metadata schema reference

### 10.1 Tables (per project unless noted)

| Table | Key | Value | ~bytes/entry |
|---|---|---|---|
| `chunk_index` | fingerprint [32] | plain_len u32, comp_len u32, codec_id u8, codec_params u8, cipher_id u8, fingerprint_alg u8, kek_version u32, nonce [24], wrapped_dek [72], plain_hash [32], location u64, created_at u64, retry_cold bool | ~200 |
| `chunk_refs` | fingerprint [32] | refcount u64 | 40 |
| `quarantine` | fingerprint [32] | quarantine_until u64, reason u8 | 41 |
| `manifests` | object_id [32] ‖ generation u64 | ManifestRoot (postcard) | ~200 + root |
| `manifest_segments` | object_id ‖ seg_no u32 | packed entries × ≤ 4096 | ≤ 180 KiB |
| `keks` (domain-wide) | project_id ‖ kek_version | RK-wrapped KEK [72], state u8 | ~90 |
| `access_clock` | object_id [32] | last_access_hbucket u32, freq u8 | 37 |
| `access_by_bucket` | hbucket u32 ‖ object_id [32] | () | 36 |
| `dedup_stats` | project_id | counters + 15 m/1 h rings | ~1 KiB |
| `topdup` | project_id | Misra-Gries top-64 {handle, refcount, dup_bytes} | ~3 KiB |
| `gc_receipts` | receipt_id | signed receipt record | ~300 |

### 10.2 Footprint

At 100 M chunks (≈ 400 TiB logical at 4 MiB average): ~20 GB raw index / ~30 GB with B-tree
overhead on disk; < 256 MB RAM (bloom + 2Q, §3.3). At 1 M objects: metadata ≤ 512 MiB
(acceptance-gated, §15).

### 10.3 Backends

Hub mode: these schemas live in RedlineDB next to the rest of the forge state. Standalone
mode: the same schemas in a local redb 2.6 database at `<data_root>/meta/shard.redb`.

### 10.4 Adoption

Because the on-disk chunk/shard format and the schemas are identical in both modes, a
standalone store is adoptable by a hub later: `jain-shardd enroll --adopt` streams local
manifests/KEK-wraps into RedlineDB (re-wrapped under the hub RK), registers the mounts as the
node's shard lanes, and flips the metadata authority — a receipt-producing, resumable import,
not a copy.

## 11. Migration from the SCQ v2 artifact store

Convert, don't keep: the legacy format (fixed-offset chunks under one bare master key, no
compression, no dedup) is a dead end as a first-class citizen.

- **Phase 0 — keys.** RK created (`JSHRK1`); the old bare 32-byte master key is registered as
  the `legacy` project's KEK v0, wrapped by the RK. A `LegacyReader` serves existing artifacts
  unchanged (nonces and `artifact_id ‖ offset` AAD from the existing redb records).
- **Phase 1 — cutover for new writes.** All new writes take the jain-shard path immediately.
  Reads check new manifests first, fall back to `LegacyReader`.
- **Phase 2 — background conversion.** `MigrateArtifactV1` WorkUnits stream-decrypt legacy
  chunks → run the full new write path (CDC, dedup, compress, encrypt) → verify the
  whole-object SHA-256 equals the existing `ArtifactId` → publish the manifest under the
  **same id** → quarantine the legacy `.xchacha` files → GC receipt. Callers see the same ids
  before, during, and after — zero visible change.
- **Phase 3 — retirement.** After receipts cover every legacy artifact, `LegacyReader` is
  deleted and the v0 KEK wrap destroyed — cryptographic erasure of the old format.

Rollback at any point before Phase 3 is trivial: `LegacyReader` still exists and legacy files
are only quarantined, not gone, until their receipts close.

## 12. Shape and packaging

### 12.1 Crates and homes

- `jain-smartcluster/crates/jain-shard/` — the engine **library**: chunker, codecs, crypto,
  manifests, dedup index, GC, stripe/erasure, tier state machine, vigilance, plus a `service/`
  module holding the verb handlers every embedder shares. Cargo features: `standalone` (redb
  metadata backend), `hub` (fabric client), `http` (read-only HTTP surface). No `no_std` core
  — the engine's whole job is file I/O and threads; `no_std` buys nothing and taxes every
  contributor.
- `jain-smartcluster/crates/jain-shardd/` — the **multi-call single binary** (daemon + client
  subcommands), a composition root of well under a thousand lines over
  `jain_shard::service::ShardService`.

Pre-merger home is `jain-smartcluster` — it owns SCQ v2 and the artifact store this engine
supersedes (shared types, shared crypto stack, one lockfile), it already has the repo
infrastructure (deny.toml, pinned toolchain, CI, `.jankurai/`), and adding a new *repo* would
mutate the family manifest authority — exactly the class of change the release lanes gate. A
new crate in an existing active repo rides the normal reviewed-PR flow. In the merged tree,
`jainnode` links the same library and spawns the same `ShardService`; `jain-shardd` remains
the storage-only composition root. One implementation of every verb, three deployment shapes.

### 12.2 Static musl is viable — and gated

The recorded family caveat (scqd links glibc because its pidfd/cgroup dependency does not
build on musl, so scqd rides distroless) does **not** apply here: a storage daemon does file
and network I/O only — no pidfd, no cgroup writes, no process supervision. Every dependency in
the shardd graph builds on `x86_64-unknown-linux-musl`. CI asserts static-ness (`ldd` refusal)
and the size gate.

### 12.3 The scratch container

```dockerfile
# syntax=docker/dockerfile:1
ARG RUST_IMAGE=rust:1.96.0-alpine3.23@sha256:PIN-FROM-CONTAINER-BASES-LOCK
FROM ${RUST_IMAGE} AS build
RUN apk add --no-cache musl-dev
WORKDIR /source
COPY . .
RUN cargo build --locked --release --target x86_64-unknown-linux-musl -p jain-shardd \
 && test "$(stat -c%s target/x86_64-unknown-linux-musl/release/jain-shardd)" -lt 12582912

FROM scratch
COPY --from=build /source/target/x86_64-unknown-linux-musl/release/jain-shardd /jain-shardd
USER 65534:65534
VOLUME ["/data"]
HEALTHCHECK --interval=30s --timeout=3s --start-period=2s --retries=3 \
  CMD ["/jain-shardd", "probe", "--quiet"]
ENTRYPOINT ["/jain-shardd"]
CMD ["serve"]
```

- Image contents = **exactly one file**. No libc, no shell, no /etc, no tzdata; standalone
  mode is UDS-only by default; hub mode carries the family trust anchors compiled into the
  binary (consistent with enrollment-token bootstrap) rather than a filesystem cert store.
- Size: target < 10 MiB stripped (`strip = "symbols"` release profile), hard CI gate 12 MiB
  (the `test -lt` above).
- `container-bases.lock` gains one pin (`rust:1.96.0-alpine3.23` by digest), changed together
  with the Dockerfile per the lock's own rule; `FROM scratch` is already lock-exempt. If the
  release lane's no-fetch discipline is read to forbid build-time `apk add`, the fallback is a
  derived pinned builder image with musl-dev pre-baked (recorded open item, not blocking).
- Pipeline: digest-pinned in compose; syft SBOM + grype + cosign receipts like every appliance
  image. Compose service: `read_only: true`, `cap_drop: [ALL]`,
  `security_opt: [no-new-privileges:true]`, `tmpfs: [/run/jain-shard]`,
  `volumes: [shard-data:/data]`, never host-published. Standalone/edge:
  `docker run --read-only --cap-drop ALL -v /mnt/pool:/data <image>@sha256:… serve`.

### 12.4 Storage attachment — point it at a mount and it works

One data root `/data` by default. Multi-mount: `/data/0` … `/data/N` are auto-detected as
distinct mounts (differing `st_dev`), each an independent shard lane and failure domain with
its own statvfs-discovered budget; `[mounts]` in `shard.toml` can override per mount. Global
floor `min_free_pct = 5` — the daemon never fills a mount past the floor (the runaway-drill
invariant). No device probing, no udev, no capabilities: the daemon uses exactly what is
bind-mounted to it. Zero config (`jain-shardd serve` with no file) = standalone mode, `/data`,
statvfs budgets, auto cache sizing. A missing `/data` is a hard, explicit startup error
(exit 9) — never a silent fallback.

## 13. CLI and API

### 13.1 CLI

clap 4 derive, scq conventions: `#[command(name = "jain-shardd", version, about)]`; globals
`--socket <PATH>` (env `JAIN_SHARD_SOCKET`, default `/run/jain-shard/shardd.sock`), `--json`,
`--config <PATH>` (env `JAIN_SHARD_CONFIG`, default `/data/shard.toml`). One multi-call binary
so the scratch image stays one file; in hub mode the family `jain` CLI proxies the same verbs
(`jain data put --profile durable` ↔ `jain-shardd put --profile durable`). Destructive verbs
are **dry-run by default** and require `--apply`, emitting JSON receipts
(`{schema_version: "jain.shard.receipt/v1", operation, mode, timestamp_unix, status, …}`)
under `<data_root>/receipts/`; release-drill receipts aggregate under
`docs/release-evidence/<version>/` per control-plane convention.

| Verb | Key flags | Dry-run/apply | Exit codes |
|---|---|---|---|
| `serve` | `--data-root <P>` (repeat), `--mode auto\|standalone\|hub`, `--hub-url`, `--http ADDR:PORT` | — | 0/1/9 |
| `probe` | `--quiet`, `--offline` | — | 0/1 (HEALTHCHECK) |
| `doctor` | `--json` | — | 0/1/7 |
| `put <file\|->` | `--project`, `--profile scratch\|standard\|durable\|critical`, `--ttl`, `--label K=V` | immediate | 0/5/6/7 |
| `get <id>` | `-o FILE\|-`, `--range A-B`, `--verify` | — | 0/3/4/7 |
| `cat <id>` | `--range A-B` | — | 0/3/4 |
| `stat <id>` / `ls` | `--project`, `--tier`, `--prefix`, `--limit` | — | 0/3 |
| `rm <id>` | `--apply` | **dry-run default** | 0/3/6 |
| `stream create/append/tail/ls` | `--project`; tail: `-f`, `--offset` | append immediate | 0/3/5 |
| `tier status [id]` / `pin <id> --ttl` / `unpin` | | pin immediate | 0/3 |
| `tier demote <id> --to warm\|cold` | `--apply` | dry-run (rewrites data) | 0/3/4 |
| `cache stats` | | — | 0 |
| `dedup report` | `--project`, `--top N` | — | 0 |
| `growth top` / `growth alerts` | `--window`, `--by project\|stream\|source` | — | 0 |
| `scrub [ids…]` | `--full` | read-only | 0/4 on corruption |
| `repair <id>\|--all` | `--apply`, `--throttle MBps` | dry-run | 0/4/8 |
| `reclaim` / `gc` | `--apply`; gc: `--grace DUR` | dry-run | 0/5 |
| `keys status` / `keys rotate` | rotate: `--apply` | dry-run | 0/6 |
| `config show` (`--effective`) / `config check <f>` | | — | 0/9 |
| `receipts ls/show` | `--op`, `--since` | — | 0/3 |
| `enroll` | `--hub URL`, `--token-file`, `--adopt` | immediate | 0/6/8 |
| `status` | `--json` | — | 0/7 |

Exit codes: 0 ok · 1 runtime · 2 usage · 3 not-found · 4 integrity failure · 5 capacity/floor
refusal · 6 auth denied · 7 daemon unreachable · 8 hub/fabric error · 9 config invalid.

### 13.2 API

- **Local**: length-prefixed bincode frames over UDS, handshake `jain.shard.uds/v1` with
  version negotiation; streaming frames for `get`/`tail`; socket dir 0700, socket 0660.
- **Hub**: the existing fabric mTLS node protocol, extended with a `storage` capability:
  `jain-shardd enroll` presents a worker-style enrollment token and advertises per-mount shard
  budgets; placement, repair, scrub, and tier WorkUnits arrive as fabric messages. Zero new
  TLS/crypto admissions — rustls/ring/chacha20poly1305/redb are already in the lockfile.
- **HTTP** (feature `http`, default **off**, `[http] listen = ""`): a minimal read-only
  HTTP/1.1 surface for humans and tools in standalone mode — `GET/HEAD /o/<id>` with `Range`,
  `GET /healthz`, `GET /metrics.json` (a JSON counter snapshot; no Prometheus, per family
  law). Implementation: `httparse` + hand-rolled responses over tokio; hyper is rejected — it
  drags http/http-body/h2/tower-service into a near-zero-deps binary for two GET routes. No
  writes over HTTP, ever.
- **Explicitly NOT S3-compatible.** SigV4, buckets, ACLs, and multipart are a foreign auth
  model and a giant compat surface; the family's consumers speak the fabric and the UDS
  protocol with grants and mTLS. If an S3 gateway is ever wanted it is a separate optional
  adapter binary, never the core.
- **Auth**: standalone = UDS filesystem permissions + optional bearer token
  (`[auth] token_file`, required for HTTP if enabled); hub = fabric mTLS identity, grants
  evaluated hub-side, short-lived signed chunk capabilities for v1.1 peer reads.

### 13.3 Config — `shard.toml` (`jain.shard.config/v1`)

```toml
schema = "jain.shard.config/v1"
[node]        # mode = "auto" | "standalone" | "hub"; data_roots = ["/data"]
[mounts]      # per-mount budget_bytes / min_free_pct; global floor min_free_pct = 5
[cache]       # hot_bytes = "auto"; pin_max_bytes = 0; mlock = "try"
[tiers]       # cold_after_days = 7; cold_quality = 10; scan_interval_hours = 6
              # max_migrate_mibps = 40; promote_hits = 3; promote_window_hours = 72
[durability]  # default_profile = "standard"
[dedup]       # alert_ratio = 0.30; alert_min_bytes = "10GiB"; report_interval_hours = 168
[growth]      # fast_halflife_s = 10; slow_halflife_s = 600; accel_ratio = 8
              # min_trip_rate_mibps = 8; min_trip_bytes = "256MiB"
              # auto_throttle = "scratch-only"; scratch_max_stream_bytes = "32GiB"
[sentinel]    # interval_s = 5; foreign_rate_mibps = 5; scan_max_stats = 10000
[pressure]    # pause_pct = 20; throttle_pct = 10; refuse_pct = 5; repair_reserve_pct = 2
[hub]         # url, enroll_token_file
[http]        # listen = ""  (off)
[auth]        # token_file
```

Env overrides `JAIN_SHARD_*` (`JAIN_SHARD_MODE`, `JAIN_SHARD_HUB_URL`,
`JAIN_SHARD_DATA_ROOTS`, …). Every default above is a frozen constant or a documented knob;
zero-config startup must work.

## 14. Dependency admission

Two allowlists exist in the family (verified from the deny.toml files, correcting an earlier
briefing that claimed MIT+Apache-only everywhere): the **control-plane strict list**
(`jain-split-ops`: Apache-2.0 + MIT (+ Unicode-3.0)) and the **product template**
(`jain-smartcluster` et al.: adds BSD-2/3-Clause, CC0-1.0, CDLA-Permissive-2.0, ISC,
Unicode-3.0, Zlib). New shard crates land against the product template; verdicts against both
are recorded because a merged-era repo may adopt the tighter list.

| Crate | Declared license | Strict list | Product template | Notes |
|---|---|---|---|---|
| `fastcdc` | MIT | PASS | PASS | pure Rust |
| `blake3` | CC0-1.0 OR Apache-2.0 | PASS (Apache arm) | PASS | `pure` feature forces Rust-only kernels |
| `reed-solomon-simd` | MIT | PASS | PASS | pure Rust, `std::arch` SIMD |
| `lz4_flex` | MIT | PASS | PASS | pure Rust |
| `ruzstd` | MIT | PASS | PASS | decode-only role (§5) |
| `brotli` | BSD-3-Clause/MIT dual | MIT arm PASS; **transitive `alloc-no-stdlib`/`alloc-stdlib` are BSD-3-only → strict-list exception needed** | PASS | the one recorded exception candidate |
| `httparse` (feature `http`) | MIT OR Apache-2.0 | PASS | PASS | — |
| `zeroize`, `argon2`, `hkdf` | MIT OR Apache-2.0 | PASS | PASS | — |
| already locked — zero admission | `chacha20poly1305`, `redb`, `rustls`, `ring`, tokio, clap, serde, bincode, tracing, nix, memmap2 | — | — | verified in jain-smartcluster/Cargo.lock |
| **reserved (OD-S4 lever)** | `zstd`/`zstd-sys` (declared MIT / MIT OR Apache-2.0; bundles C libzstd, BSD-3-Clause dual GPL-2.0) | metadata passes; **vendored C code violates the pure-Rust mandate** | metadata passes | enabling it is the recorded owner lever, never a drift |

Sequencing for hermetic release CI: (1) **admission PR** — deps added behind the shard
features, `cargo build --locked` regenerates Cargo.lock, `cargo deny check` green,
THIRD_PARTY_NOTICES updated; deps enter the lock via the reviewed flow **before** any feature
code; (2) **advisory-pin refresh** — `JAIN_PINNED_RUSTSEC_COMMIT` advanced so the pinned
RustSec DB post-dates the admitted crates; (3) all subsequent lanes build `--locked` and fetch
nothing. Toolchain stays pinned rust 1.96.

## 15. Milestones, drills, acceptance

M5 of the parent spec blows out into six drill-exited milestones. Per family law, every exit
drill becomes a permanent scheduled job.

| Milestone | Scope | Exit drill |
|---|---|---|
| **M5a** engine core + standalone | CDC, codecs, crypto, local store, redb metadata, GC | 10k-object put/get/ls round-trip incl. 0-byte and 8 GiB stream; `kill -9` mid-put ×100 → zero corruption, sweeper leaves no orphans; probe green |
| **M5b** dedup + vigilance | inline dedup, counters, ingest detector, mount sentinel | 40%-duplicate corpus → reported savings within ±2%; duplication alert fires. **Runaway drill**: synthetic 1 GB/min appender → signal row < 1 s, alert < 30 s; free space never crosses the floor (writes refused, exit 5); `stream tail` still serves throughout. Foreign-writer drill: `dd` outside the data root → sentinel names the path < 15 s in one bounded scan |
| **M5c** tiering + hot cache | temperature state machine, S3-FIFO cache, cold recompression | Zipfian replay → working-set hit ratio ≥ 80%, RSS ≤ cache budget + 15%; scan-resistance: 10 GiB stream through a 512 MiB cache drops working-set hits ≤ 5 points; 10k objects demoted cold and promoted back → byte-identical (BLAKE3), receipts present, dedup map untouched |
| **M5d** hub mode + erasure | enroll, HRW placement, repair/scrub WorkUnits | node-loss at every advertised profile → all objects readable + re-repaired; bit-rot (flip shard bytes) → scrub detects, point-repairs, receipt; topology degradation → honest (f+1)-replication, surfaced with reason |
| **M5e** keys + security posture | RK/KEK/DEK, rotation, capability auth | rotation under live reads → zero read failures; stolen-disk test: raw shards + metadata minus keys reveal nothing (entropy + known-plaintext checks); erasure drill: purge → cache holds zero project entries before the receipt; auth matrix — every denial produces a receipt |
| **M5f** productization + performance | scratch image, compose, acceptance table | image < 10 MiB, HEALTHCHECK healthy under read_only + cap_drop ALL; restart at 1 M objects → serving < 5 s; 24 h standalone soak (no leaked temp, no unbounded queues, no receipt loss); acceptance table green on reference hardware |

**Performance acceptance (CI-enforced on versioned reference hardware):**

| Metric | Target |
|---|---|
| Ingest, single stream (lz4 path) | ≥ 300 MB/s |
| Ingest, aggregate (8 cores) | ≥ 1 GB/s |
| Hot read p99 (cached) | ≤ 2 ms |
| Warm read p99 (NVMe local) | ≤ 10 ms |
| Cold read TTFB p99 | ≤ 500 ms |
| Cold read throughput | ≥ 200 MB/s |
| Daemon idle RSS (excl. hot cache) | ≤ 128 MiB |
| Metadata at 1 M objects | ≤ 512 MiB |
| Growth-alert latency at 1 GB/min | ≤ 30 s (signal row ≤ 1 s) |
| Duplication-alert latency | on the crossing write |

## 16. Future work — structure-aware layout (explicitly not v1)

The user's "dynamically recognize the literal structure on disk to aid faster retrieval" is
recorded as the v-next program. What v1 records now to enable it: a sampled access log
(1-in-16 reads plus all promotions; ring-buffer-bounded 256 MiB / 14 days; fields
`(ts_bucket, project, object, chunk-ordinal range, latency class)`).

The sketch, kept honest:

- **Co-access graph**: objects read within the same 60 s window by the same principal form
  weighted edges; community detection yields placement groups colocated per node — turning
  scatter reads into sequential ones.
- **Learned readahead**: Markov next-chunk mining replaces the fixed depth-2 readahead where
  the access log shows stable patterns.
- **Heat-aware packing**: chunk heat maps pack hot chunks contiguously within shard files on
  rotational backends; on NVMe the win is hot/cold segregation to cut compaction read
  amplification.
- **Honest limits**: ciphertext hides content structure by design — only *access-pattern*
  structure is minable; placement grouping must never cross a project boundary (co-placement
  is itself a side channel); everything remains hub-owned placement, no new consensus.

## 17. Requirement trace

| Owner requirement | Where satisfied |
|---|---|
| World-class encryption at rest | §4 (XChaCha20-Poly1305, RK→KEK→DEK, AAD, rotation, erasure) |
| World-class compression at rest | §5 codec matrix, §3.4 gate, §7.5 cold squeeze |
| "Without slowing us down" | §3.1 per-chunk parallel pipeline, lz4 ingest (§5), inline O(1) vigilance (§8), access clock without write-amp (§7.2), background-only heavy codecs (§7.5) |
| 100% Rust | OD-S4; §5 pure-Rust lineup; §14 (zstd C reserved behind the owner lever) |
| Smart dedup, caught quickly, alerting | OD-S1; §3.2–3.3; §8.4 (inline counters, crossing-write alert, cluster reports) |
| Hot/warm/cold with in-memory hot cache | §7 (S3-FIFO plaintext cache, tiers, matrix §7.7) |
| Most aggressive compression on cold, untouched for days | §7.1, §7.5 (7-day default, brotli q10/lgwin24, receipt-producing migration) |
| Near-instant runaway-log detection | OD-S3; §2.4 streams; §8.1 (detection on the crossing transfer window), §8.2 sentinel |
| Scratch container, near-zero deps | §12.2–12.3 (one static file, < 10 MiB, capability-free) |
| Great CLI/API | §13 (full verb tree, receipts, UDS/fabric/HTTP, not-S3) |
| Uses the storage/mounts given to it | §12.4 (mount auto-detection, statvfs budgets, floors) |
| Structure-aware layout (future) | §16 |

## 18. Frozen constants (the Milestone-1 freeze list)

CDC min/target/max = 1/4/16 MiB · small-object EC cutoff = 2 MiB ciphertext · stripe target =
64 MiB, shard pad 64 B · per-stream memory ceiling = 128 MiB · quarantine = 72 h (scratch 1 h)
· cold_after = 7 d (min 1) · promotion = 3 hits in distinct hours within 72 h · dup alert =
ratio > 30% ∧ > 10 GiB · growth trip = fast > max(8×slow, 8 MiB/s) ∧ stream > 256 MiB ·
sentinel = 5 s cadence, 5 MiB/s × 15 s, ≤ 10k stats/scan · pressure ladder = 20/10/max(5%,
2 GiB) + 2% repair reserve · hot cache = clamp(5% MemTotal, 64 MiB, 2 GiB) embedded/hub,
clamp(25% MemTotal, 64 MiB, 8 GiB) standalone · incompressibility probe = 64 KiB, ratio 0.97,
floor 512 B · migration cap = 40 MiB/s, batch ≤ 8 GiB, 2× headroom.

## Appendix R — reconciliation against `SUPER_MERGER_CODEX.md` §10

| Codex position (§10.1–10.4) | Disposition here |
|---|---|
| §10.1 fabric scope: uploads, datasets, git pack/LFS, knowledge, models, checkpoints, reports, logs, run outputs, repair evidence; Jeryu keeps ref authority | **Adopt** (§1.1); logs formalized as append streams (§2.4) |
| §10.1 one home project / storage-security domain per repository | **Adopt** — dedup and key domains are strictly per-project throughout |
| §10.2 CDC ~4 MiB | **Adopt + parameterize** 1/4/16 MiB (L1) |
| §10.2 dedup before encryption, HMAC-SHA-256 fingerprint | **Adopt dedup; amend PRF to keyed BLAKE3** (L4) |
| §10.2 zstd level 3 | **Amend** to codec-agile matrix, pure-Rust v1 (L7, OD-S4) |
| §10.2 per-chunk DEK / versioned project KEK / AAD binding / no nonce reuse | **Adopt** (§4); dedup_key wrap-parent clarified (L3) |
| §10.2 rotation rewraps without bulk rewrite; cryptographic erasure + tombstones; residual-recovery-window reporting | **Adopt** (§4.2); tier migration classified as declared algorithm-migration-class rewrite (L8) |
| §10.2 streamed manifests, caps removed, bounded memory | **Adopt** (§2.3, §3.5) |
| §10.2 shared_library promotion into a separate domain | **Adopt unchanged** (out of scope here; no amendment) |
| §10.3 profile table balanced/protected | **Rename with aliases** (L6) |
| §10.3 RS 4+f else f+1 | **Amend** to adaptive-k (L5) |
| §10.3 requested-vs-achieved display, attested failure domains, protected floors, Milestone-1 freeze | **Adopt verbatim** (§6.1, §18) |
| §10.3 equality-oracle rules for dedup | **Adopt verbatim** (§8.4) |
| §10.4 repair = fenced, temp-namespace, atomic publish; deletion manifest-driven + receipts | **Adopt** (§9); reused as the tier-migration choreography (§7.5) |

Where this document and Codex §10 are read together, the narrower requirement governs; a
genuine conflict is resolved by reviewed amendment and a superseding ADR before any
implementation lane opens. **Codex is invited** to review this specification
decision-by-decision or to land a parallel `SUPER_MERGER_STORAGE_CODEX.md`, per the SUPER
MERGER parallel-spec precedent.

---

*End of specification. No implementation is authorized by this document. Identity: SHA-256 of
this file as recorded in the `UPGRADE_CHAT.md` documentation-lane NOTE of 2026-07-17.*
