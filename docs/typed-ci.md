# Typed CI orchestration

`splitctl` exposes three closed, fail-closed CI interfaces:

```text
splitctl ci-plan --repo NAME --head SHA --base SHA --profile presubmit|release-full
splitctl ci-run --plan PATH --receipt PATH
splitctl ci-performance --evidence-root PATH
```

Only the root broker may create a plan or select its profile. A plan binds the
full head, tree and protected-main base; the authority manifest and contract
schemas; tracked lockfiles and toolchain files; physical executables; lane
commands, dependencies, obligations and outputs; coverage scope; cache policy;
and resource ceilings. Abbreviated SHAs, a stale `origin/main`, dirty or linked
checkouts, symlinks/gitlinks, duplicate obligations, unknown lanes, cycles, or
an unowned profile fail before a command starts.

`presubmit` routes exact changed paths through `agent/test-map.json`, adds
changed Rust packages and their reverse dependents, affected Node packages,
contract consumers, static security and changed-surface coverage. Lockfile,
CI/control, contract, generated-zone, source-policy, or control-plane changes
automatically widen to `release-full`.

`release-full` represents each required, Rust build/test, security, complete
coverage, compatibility, conformance, contract-consumer, Jankurai, artifact and
repository-specific obligation once in the typed DAG. Rust tests use
`cargo nextest`; coverage uses `cargo llvm-cov nextest`. Independent lanes run
concurrently; failed-lane dependents are canceled with their own durable log
and result, while unrelated lanes finish. Release-full always receives a fresh
Cargo target, offline mode and no compiler cache. Presubmit execution requires
the reviewed root-installed 40-GiB local-only sccache boundary and refuses to
run if that custody is absent.

The worker accepts only a root-owned, single-link, non-writable plan inside a
distinct network namespace with no live interfaces or routes. It materializes
the exact head through an automatically removed `git clone --no-local`
standalone checkout (never a Git worktree), consumes the broker-staged offline
Cargo home only after rejecting ambient registry credentials, and writes only
inside the broker-owned writable root. Every lane produces
`jain.ci-lane-result/v1`; the sealed aggregate uses `jain.host-ci-result/v6` and binds a
`jain.host-ci-evidence/v6` envelope, phase timings, CPU, peak RSS, I/O, cache
rate and critical path. This implementation is intentionally
`shadow-equivalence`: `publication_allowed=false` is closed into every plan,
result and host-evidence record. It cannot publish `<repo>/required` until an
independently reviewed release-full equivalence change replaces that contract.

## Calibration and health

Root-only shadow calibration plans additionally accept:

```text
--calibration --build-jobs 4|8|16|24|32 --fleet-concurrency 2|4|6|8
```

`ci-performance` scans physical, bounded evidence recursively. A calibration
setting is valid only after representative SplitOps, Jain Web, SmartCluster and
Redline Core samples have at least 20 GiB available memory, no OOM, no swap-in,
I/O wait below 15%, and load below 96. A successor setting must improve p95 by
at least 5%; the fastest valid setting wins, with lower job and fleet counts as
tie-breakers. CI health remains calibrating until at least 20 valid samples per
profile establish p95 at or below five minutes for presubmit and 15 minutes for
release-full.
