# P2-SAFE: uncertainty, fencing and unwind contract

Status: **open safety defect, reproduced; no production fix in this kickoff**.
Owner: storage agent; reviewer: integration owner. Base is
`af20826311a067a51383c382013612545156df61` plus preserved/current dirty changes.
The implementation must remain Rust. No Python storage or transaction runtime is
proposed. Claims are limited to the test and this document; production WAL hooks
and shared interfaces require the next exclusive claim.

## Observed failure and reproduction

Run from the canonical checkout:

```sh
rtk cargo test -p redlinedb-kernel --locked --features failpoints \
  --test strict_commit_faults -- --nocapture
```

This is a real failing safety assertion, not an ignored test or a successful
assertion that a known bug exists. Current result: **exit 101, zero passes, one
failure, zero ignored**. Raw log and exact exit are retained in
`target/compatibility-phase2/strict-commit-faults.log` and `.exit`.

The fixture commits `old` in CSN 1, updates the same row to `new`, then injects a
panic at `engine::commit::before_publish`. This point is **after successful Strict
WAL flush**. It catches the panic in a child process and observes:

| Observation | Current result |
|---|---|
| New transaction admission | Succeeds: engine is not fenced |
| Interrupted writer status | Aborted |
| Read of interrupted update | `old` |
| Competing write on that row | LockTimeout: abandoned row lock remains |
| Later unrelated transaction | `Committed(Csn(3))` returned |
| Fresh read of that acknowledged later row | None |
| Commit frontier | Published CSN 1, one pending CSN |
| Child exits without destructors; parent reopens | Interrupted row is `new`; later row is `later` |

The child exits 42 on unsafe continued use; the parent validates recovery and
fails its safety assertion. Both processes have bounded waits. The recovered
`new` is expected at this post-flush injection point. This exposes a live/reopened
state disagreement and a later acknowledged transaction hidden behind a stranded
CSN, in addition to lock retention. It does **not** inject a returned write/fsync
error or simulate power loss.

## Current state transitions

| Stage | Resources and durability | Failure behavior today |
|---|---|---|
| Active mutation | Txn owns row locks; WAL mutation records may already be queued | Txn lifecycle Drop aborts status/unregisters snapshot, but does not unlock engine rows |
| Optional catalog append | Pending schema serialized to logical WAL | Early `?` can leave the same transaction-resource cleanup gap |
| Commit append | Coordinator reserves CSN under its mutex, enqueues Commit, requests writing, wakes writer | `append_commit` can return Err; engine aborts/releases locks, but reserved-CSN ownership is not returned on an error after reservation |
| Strict barrier | `flush_until(end_lsn)` waits on durable frontier; writer may already have written commit | Returned Err cancels CSN, marks Aborted, releases locks, closes Txn, returns ordinary error |
| Durable before publication | Commit WAL has passed sync; row/catalog/index publication is pending | Panic drops Txn, aborts status but retains row locks and pending CSN; no fence |
| Publication | Tx status published, then schema/index handles, then locks released and Txn closed | Mid-publication panic can leave partial local publication; no shared recovery fence |

`append_commit` wakes the writer **before** entry to `flush_until`. The latter's
comment that its hook runs before signalling must not be read as a guarantee
that no commit bytes have been written. A valid commit record may already be
visible to recovery when any subsequent barrier reports an error.

The WAL writer catches returned `write_encoded`/`flush` errors, sets coordinator
failure to the static string `wal writer failed`, wakes waiters and exits. This
already prevents subsequent WAL append/barrier success through that coordinator.
It discards original errno, failure stage and written/durable frontiers. It does
**not** fence engine begin/read or coordinate all outstanding transactions.
A writer-thread panic bypasses this failure publication and can strand waiters.

`TxnLifecycle::drop` only changes transaction status/snapshot registration.
Reserved CSNs belong to a separate frontier, and locks belong to the engine lock
manager. Therefore a lifecycle drop is not sufficient commit cleanup. Recovery
reconstructs committed status from valid WAL Commit records; the in-memory abort
is not a durable revocation. Checkpoint/vacuum/pruning must not run over this
inconsistent live state and erase recovery evidence.

## Proposed minimum Rust contract

The following is a design proposal requiring independent review, not implemented
behavior. Keep storage format unchanged in the initial fencing slice.

1. Give each commit attempt an owned guard containing Txn/row locks, reservation
   ticket, commit end LSN, and a monotonic phase: Active → CommitEnqueued →
   BarrierSucceeded → Published. Reservation cancellation before a commit record
   is enqueued must be exception-safe; the coordinator must not lose the ticket
   if record construction/enqueue fails. No double cancellation/publication.
2. Introduce an engine-shared, terminal `RecoveryRequired` health state. Once a
   commit may be in WAL, any barrier failure or unwind before complete publication
   installs that state **before** cleanup exposes resources to other operations.
   A guard Drop must not panic or require a healthy/poison-free mutex to set the
   fence. Do not use clearing the CSN hole to resume this live engine.
3. New operations on every connection sharing the engine reject with the same
   recovery-required cause. Existing operation permits must coordinate with the
   fence before returning/publishing; a check only at `begin` is insufficient.
   Gate reads, writes, commits, schema operations, checkpoint, backup and pruning.
   The conservative initial contract permits diagnostics and close only after
   fencing; it does not promise continuing old snapshots.
4. Preserve an immutable WAL failure record: operation, OS error kind/raw errno,
   affected LSN range, last known written/durable frontier, and commit identity
   when known. All waiting committers are woken and receive an outcome appropriate
   to their own frontier. A terminal writer guard also reports panic/early exit.
   This must cover group-commit and shutdown paths, not only the normal writer loop.
5. Distinguish a definitely unqueued commit rejection from an **indeterminate**
   commit after enqueue. Surface indeterminate transaction identity/LSN and
   recovery-required status in a structured Rust error/outcome. Never claim
   rollback, never automatically retry, and never publish unflushed data to make
   local state resemble a possible recovery result.
6. Release abandoned process-local locks/registration exactly once only under
   the terminal fence, and drain/cancel waiters without acknowledging success.
   Quiesce the writer and prevent maintenance publication/pruning; retain WAL and
   pre-existing durable checkpoint. Review shutdown flushing explicitly rather
   than assuming dropping Arc restores a consistent state.
7. Reopen is a new, exclusive engine generation after all old handles/operations
   are drained or invalidated. Recovery decides from valid persisted WAL; an
   uncertain operation may be present or absent. No live in-place engine reuse,
   no direct uncoordinated replacement while old handles remain. Process admission
   must protect this handoff. A storage diagnosis must not silently repair corrupt
   coordination evidence or truncate potentially relevant WAL.

Existing `MaybeCommitted` is generated by a test path **after successful barrier**
and currently calls `finish_commit`. Merely returning this enum from the barrier
error branch would neither fence the engine nor establish safe metadata/counter
state. Calling `finish_commit` there would publish unflushed data. Neither is an
acceptable fix. Post-flush ambiguous API delivery and failed-durability uncertainty
must carry distinguishable facts, even if an application-facing code is shared.

## Shared callers and file claims for implementation

| Surface | Required coordinated change |
|---|---|
| `engine/runtime/commit.rs`, `engine/tx.rs`, `engine/tx/status.rs` | Commit guard, reservation ownership, terminal cleanup; preserve successful flush-before-publication |
| `wal/manager/storage/write.rs` | Separate returned-error hooks around write/sync; keep existing skip-sync fault unchanged |
| `wal/manager/coordinator/{writer,helpers,methods,control}.rs` | Preserve failure cause, wake all waiters on return/unwind, stop unsafe continuation; account for reservation failure |
| `engine/mod.rs`, `error.rs`, runtime/read/catalog/maintenance entrypoints | Shared health state, operation fencing and structured uncertainty contract |
| `sql/connection/session.rs` | Explicit COMMIT currently restores sequence snapshot on generic Err; distinguish uncertainty and invalidate dependent state/connection |
| `sql/exec/mod.rs` | Autocommit currently restores sequence snapshot on generic Err and on MaybeCommitted; no replay or claim of rollback after uncertainty |
| `kernel/engine/catalog_ops/index.rs` | Rebuild currently turns MaybeCommitted into generic CorruptWal; preserve recovery-required cause |
| `ffi/util.rs` and statement/connection error paths | Map structured I/O uncertainty deliberately, preserve extended diagnosis, prohibit use/retry of fenced connection |
| CLI, Rust API, attachment/backup owners | Same outcome/fence, no successful close/checkpoint or per-connection-only workaround |

These are prospective coordinated claims. This kickoff changed none of them.

## Next implementation slice and acceptance

First claim `wal/manager/storage/write.rs` exclusively and add distinct failpoint
closures returning real `Error::Io` before write, after full write, before sync,
and after successful sync. A separate partial-write hook must write a bounded
prefix before returning error. Preserve existing `wal::flush` semantics: its
`return` intentionally reports success without sync and cannot stand in for an
I/O Err. Existing closureless `wal::flush_until`/`wal::write_encoded` hooks likewise
cannot supply typed returned errors. Do not combine these production hooks with
an unreviewed outcome change.

Use those hooks to land bounded **failing** acceptance tests for returned errors;
then implement the guard/fence/structured-error contract across the callers above.
Keep the current panic test failing until the fence works, then extend its success
branch to verify all gated APIs and dependents rather than only `begin`.

Acceptance must cover:

- No-write error before commit enqueue: no recovery commit and all local resources
  released; a definitely aborted classification requires proof of no queued marker.
- Partial/full commit write then Err; sync-before/sync-after-success Err: no
  successful acknowledgement, no premature publication, explicit uncertainty and
  terminal shared fence. Reopen may recover a complete valid commit; invalid tail
  does not invent one. Repeat with inserts, updates, deletes, indexes and DDL.
- EIO, ENOSPC and injected allocation failure at the applicable boundaries; retain
  original cause. Injection models control flow and does not prove real hardware
  durability or torn-write atomicity.
- Multiple writers sharing one flush: all waiters terminate within deadlines;
  no commit above the known durable frontier acknowledged. Independent connections
  and already-open transactions must observe the same fence.
- Caller panic at prebarrier, postbarrier and partial publication; writer-thread
  panic; lock/CSN/snapshot cleanup cannot strand readers or return later hidden
  successful commits. No unwinding across C; mutex poisoning is not the API fence.
- Close/checkpoint/vacuum/pruning after fault cannot destroy recovery evidence;
  exclusive recovery restores catalog, indexes, rows and sequences consistently.
- Existing successful Strict acknowledgement/reopen and flush-before-visibility
  tests continue passing. Normal/UnsafeDev behavior is explicitly tested, without
  importing their weaker guarantees into the Strict result.

SAFE-01 remains incomplete until these tests, shared-caller integration and
independent review pass. This kickoff intentionally demonstrates an unresolved
safety failure instead of reporting a green durability gate.

Evidence hashes (SHA256):

- `target/compatibility-phase2/strict-commit-faults.log`: `1a849809f7ce9c1f6edb6eb15b0d087ba376c4959a446415257bd18a02c02b10` (test exit 101).
- `target/compatibility-phase2/strict-commit-faults.exit`: `39b8dc3fc8b44765c8e6f1adee04c5b465e555ab791cc42d0d9e810d5b64297c`.

`rtk proxy rustfmt --edition 2024 crates/kernel/tests/strict_commit_faults.rs`
and scoped `git diff --check` exit 0. No production code or release changed.
