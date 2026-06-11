# RedlineDB Hub — Release Process

## How releases work

RedlineDB releases are built from a tagged `redline-core` commit. The hub repo
(`neverhuman/RedlineDB`) hosts the binary assets and the installer.

## Cutting a release

1. **Tag `redline-core`** — e.g. `git tag v1.2.3 && git push origin v1.2.3`
   (must pass `redline-core` CI gate first).
2. **Tag the hub** — push the same version tag to this repo:
   ```bash
   git tag v1.2.3
   git push origin v1.2.3
   ```
3. **CI builds** — `.github/workflows/release.yml` triggers, builds binaries for all
   target platforms via `ops/ci/release.sh`, and publishes to GitHub Releases.
4. **Verify** — confirm all platform assets appear on the Releases page and the
   SHA-256 checksums are uploaded.

## Cost and budget

Each release run builds two platform targets (linux-x86_64, linux-aarch64) on
GitHub-hosted runners. Budget per release: ≤ 2 × 60 min = 120 runner-minutes on
standard GitHub Actions billing.

The release workflow is gate-protected by `concurrency: cancel-in-progress: false`
to prevent partial releases.

## Rollback

To retract a broken release:

1. Delete the GitHub Release (do not delete the tag — it preserves audit history):
   ```bash
   gh release delete v1.2.3 --repo neverhuman/RedlineDB --yes
   ```
2. Re-run the release workflow from a fixed `redline-core` ref:
   ```bash
   gh workflow run release.yml --field core_ref=<fixed-sha>
   ```

## Version source

The authoritative version is declared in [`VERSION`](../VERSION) at the repo root.
Tags must match the `VERSION` file exactly (e.g. `VERSION` contains `1.2.3`, tag is `v1.2.3`).
The build records which `redline-core` ref was used in the release notes.

To bump the version:
```bash
echo "1.2.3" > VERSION
git add VERSION && git commit -m "chore: bump version to 1.2.3"
git tag v1.2.3
git push origin HEAD v1.2.3
```
