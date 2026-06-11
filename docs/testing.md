# RedlineDB Hub — Testing and Launch Gate

## CI gate

Every PR and push to `main` runs the full CI gate:

```bash
just check          # local (identical to CI)
bash ops/ci/pr-ci.sh  # direct
```

The gate is defined in `.github/workflows/ci.yml`. It runs:
- shellcheck on all shell scripts (`install.sh`, `ops/ci/*.sh`, `scripts/*.sh`)
- JSON syntax validation on `family.json`
- Pointer integrity checks (README, FAMILY.md, family.json are in sync)
- Thin-hub invariant (no engine code leaked in)
- Contract drift check (`contracts/install-url.txt` vs `install.sh`)
- Security lane (`ops/ci/security.sh`)
- Jankurai advisory audit

## Launch gate — what must be green before a release

The following must pass before tagging a release:

1. **PR CI gate** (`.github/workflows/ci.yml`): all jobs green — shellcheck, pointer checks, contract drift, security lane.
2. **Security scan** (`.github/workflows/security.yml`): gitleaks clean, no blocked artifacts, no committed secrets.
3. **Release workflow smoke** (`.github/workflows/release.yml`): dry-run passes, binary builds on all target platforms.
4. **`install.sh` verification**: manually test `REDLINE_VERSION=<tag> bash install.sh` on a clean machine; verify binary is executable.
5. **SHA-256 checksums**: verify the published `.sha256` files match the downloaded assets.
6. **Backups**: source is git-backed at GitHub; release artifacts are stored on GitHub Releases CDN with redundancy. Tag the commit before publishing so the exact source is pinned.
7. **Monitoring**: after publishing, verify the GitHub Release page loads, all assets appear, and download links resolve. Check CI badge on `main` is green.
8. **Rollback procedure**: if a release is found to be broken after publishing:
   - Delete the GitHub release (do NOT delete the git tag yet — it is the reference).
   - Push a fixed commit, re-tag with the same semver patch increment.
   - Re-run the release workflow and publish the corrected artifacts.
   - Post a security advisory if the broken release contained a security regression.
9. **Abuse controls**: install.sh validates the downloaded binary via SHA-256 checksum before executing. GitHub Actions secrets are scoped per-repo. CI pinned to tagged action refs (no mutable `@main` refs).

## Proof artifacts

After a successful release, the following proof artifacts exist:

- GitHub Release page for the tag: lists all assets + checksums
- `ops/ci/release.sh` build logs (from CI artifacts)
- `.jankurai/score-history.jsonl` — version-locked jankurai score at time of release
- `ops/observability/README.md` — observable signals and repair receipts

## Cost and budget

CI gate: ≤ 12 min on ubuntu-24.04.
Security gate: ≤ 15 min on ubuntu-24.04.
Release build: ≤ 2 × 60 min (two platforms).

Total budget per release cycle: ≤ 150 runner-minutes.
Stop condition: if the release workflow exceeds 60 min per platform, it times out automatically (`timeout-minutes: 60`).

## Contract drift

The install URL contract is checked in every PR:
- Declared in: `contracts/install-url.txt`
- Checked by: `ops/ci/pr-ci.sh` (contract drift step)
- Owner: `maintainer`

See [docs/boundaries.md](boundaries.md) for the full boundary description.

## Agent repair hints

When CI fails, use this table to route to the right fix:

| Error | Root cause | Fix |
|-------|------------|-----|
| `shellcheck`: SC2xxx | Shell quoting / syntax | `shellcheck <file>` locally; fix the flagged line |
| `family.json` parse error | Invalid JSON | `python3 -c "import json; json.load(open('family.json'))"` |
| Pointer check: README missing | Out-of-sync pointers | Update `README.md`, `FAMILY.md`, `family.json` together |
| Thin-hub invariant fired | Engine code leaked in | Remove Rust/SQL files; they belong in `redline-core` |
| Contract drift: install.sh URL | Dynamic variable missing | Restore `${VERSION}` in the URL; see `contracts/install-url.txt` |
| gitleaks: secret detected | Credential in source | Remove the secret, rotate it, then force-push to purge history |
| Ownerless path | New file with no owner | Add the path to `agent/owner-map.json` under the right owner |
| jankurai score below 85 | Dimension regressed | Run `just score`; inspect `target/jankurai/repo-score.md` |

Repair receipts are written to `target/jankurai/repo-score.json` after every `just score` run.
Re-run the full gate: `just check`.

**Typed error**: if `ops/ci/pr-ci.sh` exits non-zero, the first `echo "ERROR: ..."` line is the
machine-readable failure reason. Route it to the repair table above.
