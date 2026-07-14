# Interaction-volume certification

This document is the current contract for the Redline, SQLite, and PostgreSQL
interaction-volume certificate. It is a bounded, closed-loop comparison of the
checked-in seeded workload. It does not establish an envelope for untested
arrival rates, bursts, payloads, concurrency, or customer data volumes.

## Commands

The local smoke is informational and cannot authorize a release:

```sh
# The image must already be cached unless this one pull is explicitly enabled.
REDLINEDB_CERT_ALLOW_IMAGE_PULL=1 rtk just interaction-volume-smoke
rtk just interaction-volume-smoke
```

The daily profile is CI-only. GitLab invokes the runtime boundary directly:

```sh
bash ops/ci/interaction-volume-ci-entrypoint.sh daily
```

That boundary requires `CI=true`, the exact daily job name and checked-out
commit, a reachable Docker client and server, a permitted pipeline source, and
nonzero pipeline/job IDs. It writes `trigger-evidence.json`; running
`rtk just interaction-volume-daily` outside that environment fails closed.

## Runtime identity

The Docker daemon service is pinned in `.gitlab-ci.yml` and the trigger contract
to:

```text
sha256:aa3df78ecf320f5fafdce71c659f1629e96e9de0968305fe1de670e0ca9176ce
```

The owned PostgreSQL image is pinned to:

```text
sha256:786dab398303b8ce7cb76b407bb21ef2e4dfbbbd4c6abcf3d29b3130467ffdbc
```

Before benchmark work starts, Rust independently observes the live Docker
daemon, full container ID, unique run label, image ID and RepoDigest, health and
start time, read-write PostgreSQL bind, published endpoint, filesystem mount
identities, physically allocated reserve, and PostgreSQL system/start/data-dir
identity. Wrapper JSON is not sufficient by itself. Executable adversarial
cases prove that stale container IDs and substituted ports are rejected.

The CI container proves that the Docker daemon can read the job's build/cache
path before starting PostgreSQL. This is necessary for Docker-in-Docker, where a
client-visible pathname is not automatically daemon-visible.

## Read correctness

The workload uses one immutable operation plan for every engine at a given
point. Every timed point read obtains session state and its committed event
count from one statement snapshot and requires `last_seq` to equal that count.
Every timed replay carries the same committed count on every returned row and
requires exactly `min(count, 20)` rows. A session with no events must return one
explicit all-null event sentinel; an empty result is not accepted. Event ID,
session, sequence, kind, payload, and descending order are all checked.

After checkpoint and reopen, the complete session/event integrity digest must
equal the digest of the immutable operation plan. Failed or skipped checks
cannot produce a successful manifest.

## Storage containment

The canonical daily ceiling is 2 GiB across data plus WAL. The watchdog stops at
1.5 GiB, polls every 25 ms during timed work, and reserves a physically
allocated 512 MiB file on the same durable filesystem. The process also has a
1.5 GiB per-file `RLIMIT_FSIZE`. Smoke uses a 128 MiB hard ceiling, a 96 MiB
stop threshold, and a 32 MiB reserve.

Redline accounts every file below its database root. SQLite accounts its
database and optional WAL. PostgreSQL accounts its complete default tablespace
and current WAL directory. Missing required paths, traversal/metadata failures,
negative sizes, and arithmetic overflow all fail; they never become zero-byte
observations. These sizes are safety bounds only and never rank engines.

The release comparison requires all three roots and the reserve to resolve to
one non-memory host mount. Service-managed or unmatched storage can demonstrate
mechanics only.

## Termination and cleanup

The smoke CI lane runs both timeout and external-SIGTERM cases only after
PostgreSQL is the active engine and owns a live benchmark schema. Cleanup must
stop the Rust child, remove every benchmark schema, remove the owned container,
and remove the runtime directory. `cleanup.json` records each assertion.

Additional executable cases reject stale container evidence, endpoint
substitution, and a PostgreSQL baseline that already exceeds the aggregate
ceiling. All of these run in `interaction-volume-smoke`; none is waived by the
daily job.

## Receipts and release eligibility

`attempt.json` is written before the first engine run and `progress.json` is
refreshed atomically through every lifecycle phase. Completed runs are stored in
`raw-runs.json`; `manifest.json` binds its digest, the running binary, checked-out
source, live PostgreSQL observation, storage contract, and runtime trigger.

Smoke must report `mechanics_passed=true`, `status=informational_pass`, and
`release_eligible=false`. Release mode additionally requires the exact
checked-in daily profile, clean source, live owned-container provenance, shared
durable storage, a runtime-bound CI receipt, complete integrity/read checks, and
all fixed comparison criteria. The only bounded comparison boolean is
`bounded_reference_win_eligible`.

Receipts are retained below `target/ci/interaction-volume/<case>/`. GitLab keeps
smoke artifacts for 14 days and daily artifacts for 90 days.
