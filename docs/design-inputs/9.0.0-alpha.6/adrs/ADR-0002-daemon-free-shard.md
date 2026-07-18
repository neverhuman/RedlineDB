# ADR-0002: daemon-free Shard engine

Status: accepted for the compute/storage canary  
Supersedes: `SUPER_MERGER_STORAGE_CLAUDE.md` OD-S2 and every `jain-shardd` proposal

`jain-shard` is a Rust engine library and qualification CLI, never a daemon. It owns bounded
chunking, compression, encryption, erasure, and descriptor-anchored local I/O algorithms.
`jainhub` owns authoritative keys, dedup index, manifests, refcounts, placement, reservations,
tiers, receipts, and repair queues. Native `jainnode` owns FUSE, the ciphertext plane, volume
lifecycle, node-local storage execution, and cleanup.

Storage-only nodes run the same native `jainnode` with a capability-limited signed registry. Repair,
scrub, repack, rewrap, GC, and control maintenance are signed WorkUnits through SmartCluster; they
do not create a second scheduler, consensus system, or execution bypass. Standalone development
may embed the library in a non-production tool, but cannot be promoted or represented as canary
evidence.

