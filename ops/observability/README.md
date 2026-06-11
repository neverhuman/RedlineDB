# ops/observability — RedlineDB hub observability

## What's observable

The RedlineDB hub is a thin front-door (no runtime process), so observability focuses on
release health and CI signal quality.

| Signal | Where | How |
|--------|-------|-----|
| Release asset health | GitHub Releases page | `gh release view <tag>` |
| Download counts | GitHub Traffic API | `gh api repos/neverhuman/RedlineDB/traffic/clones` |
| CI gate health | `.github/workflows/ci.yml` | PR check-runs |
| Jankurai score | `target/jankurai/repo-score.json` | `just score` |
| Security scan results | `.github/workflows/security.yml` | CI artifacts |

## Repair receipts

After every `just score` run, jankurai writes:
- `target/jankurai/repo-score.json` — machine-readable score with per-dimension breakdown
- `target/jankurai/repo-score.md` — human-readable audit report

When a CI job fails, the first `ERROR[code]:` line from the job log is the machine-readable
reason. Route it to `docs/testing.md#agent-repair-hints` for the fix.
