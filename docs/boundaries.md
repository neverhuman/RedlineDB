# RedlineDB Hub — Boundaries and Data Flow

## What crosses the hub boundary

This repo is a **thin front-door**. The only runtime artifact it owns is `install.sh`,
which downloads a pre-built binary from GitHub Releases.

### Inbound (what enters this repo)

| Source | What | How |
|--------|------|-----|
| `redline-core` tag push | Triggers binary build | GitHub Actions workflow on `push: tags` |
| maintainer tag on this repo | Starts the release workflow | `push: tags: ["v*"]` in release.yml |

### Outbound (what leaves this repo)

| Artifact | Destination | Mechanism |
|----------|-------------|-----------|
| `redline-$TAG-$TARGET.tar.gz` | GitHub Releases assets | `gh release upload` in `ops/ci/release.sh` |
| `install.sh` URL | End users' machines | `curl | bash` one-liner |

## What does NOT cross the boundary

- No database access — this repo contains no SQL schema, no migrations, no ORM.
- No secrets at rest — all secrets are GitHub Actions secrets (`GITHUB_TOKEN`); none are
  checked in.
- No engine source — all Rust/C engine code lives in `redline-core`.

## Public API surface

The hub's **public API** is the binary download URL template in `install.sh`:

```
https://github.com/neverhuman/RedlineDB/releases/download/v${VERSION}/redline-v${VERSION}-${target}.tar.gz
```

This URL shape is the install contract. Changes that break the URL pattern (rename
assets, change the version scheme) are breaking changes and require a major version bump.

The URL pattern is verified in CI by `ops/ci/pr-ci.sh` (contract check step).

See [contracts/install-url.txt](../contracts/install-url.txt) for the machine-readable
contract declaration.
