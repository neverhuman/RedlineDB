# SAFE-01: preserved Strict commit patch review

Baseline: `af20826311a067a51383c382013612545156df61`, with the uncommitted
patch preserved at the beginning of the 2026-09-18 implementation cycle.
Reviewers: integration owner and independent SQL regression agent.
Disposition: **partial; not qualified for the full SAFE-01 acceptance gate**.

The removed fast path called `publish_commit`, released row locks and closed the
transaction before `flush_until`. The preserved change routes Strict commits
through the durability barrier before `finish_commit`, including ordinary DML.
This corrects the ordering on successful commits. Normal and UnsafeDev continue
using their separate configured barriers.

## Executable evidence

Run from the canonical checkout:

```
rtk cargo test -p redlinedb-kernel --locked --features failpoints \
  --test failpoint_smoke --test strict_commit_recovery --test engine_tests
```

Raw cycle log: `target/compatibility-cycle1/kernel-tests.log` (exit 0).
At review: 24 engine tests, seven failpoint tests and one process-recovery test
passed, with no ignored tests in these binaries. The integrated evidence receipt
records file hashes and the final rerun; these are dirty-tree results, not a
release qualification or a committed implementation identity.

- The preserved panic test checks that a barrier panic has not published the row.
- The new blocked-flush test observes an InProgress writer, its old committed
  value, and a competing writer's LockTimeout before releasing the barrier. It
  then checks snapshot stability, fresh visibility and row-lock release.
- The new process test commits Strict, exits without running engine destructors,
  then reopens from the parent and verifies the acknowledged value. This tests
  process exit and recovery, not loss of power or hardware caches.
- Existing concurrent group-commit/reopen tests remain in the engine test suite.

## Remaining integrity questions

`append_commit` enqueues the commit record and signals the writer before the
barrier. A returned write/fsync error therefore cannot establish that the record
will never reach recovery. The current error branch aborts locally and returns
an ordinary error; it does not explicitly represent this uncertainty or fence
continued use. This requires fault injection for returned I/O errors and a
reviewed uncertain-outcome/recovery contract before SAFE-01 is complete.

A panic bypasses the error branch's reserved-CSN cancellation and row-lock
release. Transaction lifecycle drop aborts its status but does not itself release
those engine resources. Catching a panic and continuing cannot yet be described
as safe. The existing panic fixture establishes invisibility only.

Further required evidence: actual write/fsync errors, ENOSPC, process death at
every publication boundary, catalog/index publication, continued-use fencing,
checkpoint interactions and acknowledged-commit survival under the declared
failure model. No broad durability claim follows from the limited tests above.
