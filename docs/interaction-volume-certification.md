# Interaction-volume certification

This document is the current contract for the Redline, SQLite, and PostgreSQL
interaction-volume harness. Smoke exercises one checked-in, closed-loop seeded
workload, but it is not competitive evidence and authorizes no bounded-win,
release, or customer-load claim. The latest smoke trails both reference engines.
It does not establish an envelope for untested arrival rates, bursts, payloads,
concurrency, or customer data volumes.

## Commands

The local smoke is informational and cannot authorize a release:

```sh
# The image must already be cached unless this one pull is explicitly enabled.
REDLINEDB_CERT_ALLOW_IMAGE_PULL=1 rtk just interaction-volume-smoke
rtk just interaction-volume-smoke
```

The daily profile is currently disabled and must fail closed:

```sh
bash ops/ci/interaction-volume-ci-entrypoint.sh daily
```

The release authority is host-native local Jeryu, not caller-supplied GitLab
environment variables. This repository has no checked-in canonical HTTPS
origin, CA identity, signing key, or verifier for a host-CI/Jeryu exact-head
attestation. The trigger contract therefore has `daily_enabled=false`, no
permitted source, and null authority identities; both the shell boundary and
Rust reject trigger receipts before using any caller URL. The GitLab daily job
is disabled. A future owner must add reviewed control-plane plumbing that binds
repository, exact source SHA, required check name/result, runner identity,
issue/expiry times, a non-replay nonce, payload digest, and signature to a
checked-in authority. Until then there is no runnable or passing daily lane.

## Runtime identity

The Docker daemon used by the GitLab smoke mechanics job is pinned in
`.gitlab-ci.yml` and the trigger contract to:

```text
sha256:aa3df78ecf320f5fafdce71c659f1629e96e9de0968305fe1de670e0ca9176ce
```

The owned PostgreSQL image is pinned to:

```text
sha256:786dab398303b8ce7cb76b407bb21ef2e4dfbbbd4c6abcf3d29b3130467ffdbc
```

Before benchmark work starts, Rust independently observes the live Docker
daemon, full container ID, unique run label, image ID and RepoDigest, health and
start time, read-only root filesystem, disabled log driver, bounded Docker-reported
writable layer, read-write PostgreSQL bind, published endpoint, filesystem mount
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
database and optional WAL. For the owned-container path, PostgreSQL recursively
measures apparent bytes below `PGDATA` from inside the exact evidence-bound
container, including the visible global catalogs, transaction state, temporary
files, and `pg_wal`. The container root is read-only and its Docker log driver
is disabled. Docker `SizeRw` is measured on every sample, must remain at or
below 1 MiB, and is added to the same safety cap. This scope is named
`recursive_pgdata_apparent_bytes_plus_docker_size_rw`. It is not a complete
PostgreSQL process, cluster, container, host, or persistent-footprint measure:
it excludes process memory, network buffers, bounded tmpfs mounts, image layers,
Docker daemon metadata, and files no longer reachable below `PGDATA`.
Service-managed PostgreSQL exposes only a server-reported
default-tablespace-plus-WAL subset and can never authorize a comparison.
Missing observations, negative sizes, and arithmetic overflow fail; they never
become zero-byte observations. All engine byte values are safety bounds only
and never rank engines or support a cross-engine footprint claim.

The release comparison requires all three roots and the reserve to resolve to
one non-memory host mount. Service-managed or unmatched storage can demonstrate
mechanics only.

## Termination and cleanup

The smoke CI lane runs both timeout and external-SIGTERM cases only after
PostgreSQL is the active engine and owns a live benchmark schema. Cleanup must
stop the Rust child, remove every benchmark schema, remove the owned container,
and remove the runtime directory. `cleanup.json` records each assertion. After
cleanup, the EXIT boundary atomically adds the exact `cleanup.json` SHA-256 to
`manifest.json` and fails an otherwise successful run if that binding cannot be
written and verified.

Additional executable cases reject stale container evidence, endpoint
substitution, and a PostgreSQL baseline that already exceeds the aggregate
ceiling. All of these run in `interaction-volume-smoke`; none is waived by the
daily job.

## Receipts and release eligibility

`attempt.json` is written before the first engine run and `progress.json` is
refreshed atomically through every lifecycle phase. Completed runs are stored in
`raw-runs.json`; `manifest.json` binds its digest, the running binary, checked-out
source, live PostgreSQL observation, storage contract, and the post-run cleanup
receipt.

Smoke completion reports `mechanics_passed=true`, `status=smoke_complete`,
`release_eligible=false`, and `bounded_reference_win_eligible=false`. It remains
noncompetitive even when mechanics pass. Release mode is blocked until the
authoritative host-CI/Jeryu attestation plumbing above exists; no current
receipt may claim a daily pass.

Receipts are retained below `target/ci/interaction-volume/<case>/`. GitLab keeps
smoke artifacts for 14 days and daily artifacts for 90 days.
