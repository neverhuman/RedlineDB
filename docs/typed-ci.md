# Typed CI orchestration

`splitctl` exposes three closed, fail-closed CI interfaces:

```text
splitctl ci-plan --repo NAME --head SHA --base SHA --profile presubmit|release-full
splitctl ci-run --plan PATH --receipt PATH
splitctl ci-performance --evidence-root PATH
```

Only the root broker may create a plan or select its profile. The v1 execution
contract is deliberately closed (`execution.allowed=false`): `ci-run` validates
the immutable plan and then refuses before materialization or product command
execution. Activation requires a separately reviewed protected-main successor
that integrates the existing root-owned systemd filesystem/credential sandbox,
a root create-only evidence sealer, an enforced fleet lease, a read-only
transitive-tool manifest, continuous cgroup sampling, post-run exact-tree proof,
and release-full equivalence. A plan binds the
full head, tree and protected-main base; the authority manifest and contract
schemas; tracked lockfiles, Cargo configuration and toolchain files; physical
Rust/Node/CI executables; lane
commands, dependencies, obligations and outputs; coverage scope; cache policy;
and resource ceilings. Abbreviated SHAs, a stale `origin/main`, dirty or linked
checkouts, symlinks/gitlinks, duplicate obligations, unknown lanes, cycles, or
an unowned profile fail before a command starts.

`presubmit` routes exact changed paths through `agent/test-map.json`, adds
changed Rust packages and their reverse dependents, affected Node packages,
contract consumers, static security and changed-surface coverage. Every
affected Node package must expose an exact `lane=node-test` route whose command
is `pnpm --dir <package> run test`; the planner binds the governed `pnpm`
executable directly and rejects broad `required` routes for affected Node
paths. Lockfile, CI/control, contract, generated-zone, source-policy, or
control-plane changes automatically widen to `release-full`.

`release-full` requires an explicit repository-owned
`ops/ci/typed-required-non-test.sh` mapping. It represents each required, Rust build/test, security, complete
coverage, compatibility, conformance, contract-consumer, Jankurai, artifact and
repository-specific obligation once in the typed DAG. Each Rust qualification
configuration runs once through `cargo llvm-cov nextest`, satisfying its test
and coverage obligations together. Cross-repository consumers require one
authority-bound executable command each; a local contract-drift command is not
misrepresented as consumer execution. Missing repository mappings fail plan
creation. Presubmit likewise combines each changed Rust test and coverage run,
requires the typed Node route described above, and requires the
physical root-installed 40-GiB local-only sccache identity at plan time.

No v1 worker result is trusted or aggregatable. The reserved
`jain.host-ci-result/v6` contract requires an immutable root-owned mode-0444
result, the exact immutable plan, a root-sealed systemd boundary identity, an
actual fleet lease/slot, a root-owned transitive-tool manifest, post-run source
proof, and continuous aggregate cgroup measurements. `ci-performance` rejects
unsealed or self-asserted samples; because v1 plans cannot activate, health
honestly remains calibrating with zero accepted v1 samples. Publication remains
independently closed by `publication_allowed=false` and cannot publish
`<repo>/required`.

## Calibration and health

Root-only shadow calibration plans additionally accept:

```text
--calibration --build-jobs 4|8|16|24|32 --fleet-concurrency 2|4|6|8
```

`ci-performance` scans physical, bounded, root-sealed evidence recursively. A calibration
setting is valid only after representative SplitOps, Jain Web, SmartCluster and
Redline Core samples have at least 20 GiB available memory, no OOM, no swap-in,
I/O wait below 15%, and load below 96. Settings are evaluated in the declared
4/8/16/24/32 build-job sequence and then 2/4/6/8 fleet sequence. The first
setting is a baseline, never a winner; every accepted successor must have the
immediately preceding measured setting and improve p95 by at least 5%. The
fastest valid setting wins, with lower job and fleet counts as tie-breakers. CI
health remains calibrating until at least 20 valid samples per profile establish
p95 at or below five minutes for presubmit and 15 minutes for release-full.
