# Jankurai Local CI Authority Exception

## Surface

- Jankurai rules `HLT-000-SCORE-DIMENSION` and
  `HLT-048-CANONICAL-CI-GAP`.
- `.github/workflows/jankurai.yml`, which is a public static mirror and is not
  a Redline release gate.

## Why this is allowed

Jankurai 1.6.11 recognizes a governed audit lane only when the Jankurai command
is embedded in a GitHub Actions workflow. Redline release authority instead
runs on the 100%-local Jeryu fleet. The fleet installs the exact
protected-main Jankurai binary into its root-owned authority directory before
starting repository code. `ops/ci/lib.sh` freezes that selected executable and
revalidates its regular-file status, physical path, version, and SHA-256 before
each dispatch.

The stock GitHub runner cannot reach the local forge or its authority binary.
Making it run a release audit would require an external source fetch, a
floating artifact, privileged installation, or a false-green skipped step.
The public workflow therefore runs only the static mirror contract and cannot
emit release evidence.

These two disabled rules suppress only Jankurai's GitHub-workflow-shape false
positive. They do not relax the repository's minimum score, high-severity
failure policy, governed-binary validation, security lane, score lane, or full
local Jankurai audit.

## Owner / Expiry

- Owner: `ops` (see `agent/owner-map.json`).
- Expiry: when Jankurai can represent a non-GitHub local release authority and
  independently classify a public static mirror.
- Migration path: remove these two policy entries after upgrading the governed
  Jankurai pin and proving the local-fleet authority through the new native
  metadata.
- Proof lanes: `bash ops/ci/github-mirror-contract.sh`,
  `bash ops/ci/governed-jankurai-test.sh`, `just score`, and
  `bash ops/ci/jankurai-audit.sh` under the exact governed PATH.
