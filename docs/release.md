# Redline Central release

Redline Central keeps the Redline family's native `4.1.0-jain.N` identity.
Jain 8.0.1 consumes an immutable Redline tag by exact commit and tree checksum;
it does not rewrite this repository's package version to 8.0.1.

The onboarding candidate is `redline-central-v4.1.0-jain.4`. The tag may be cut
only after the production-readiness pull request passes `redline-central/required`,
receives an independent approval, and protected `main` advances by fast-forward.
Existing tags are immutable and corrections use the next unused `.jain.N` suffix.

## Version source

`VERSION` and `[workspace.package].version` in `Cargo.toml` must agree. The
contract lane verifies both workspace crates report `4.1.0` through locked
Cargo metadata. `CHANGELOG.md` carries the matching `.jain.N` candidate note.

## Release checklist

1. Run `bash scripts/ci-local.sh required` at the exact review head.
2. Read back governed score, security, contract, artifact, and coverage proof.
3. Obtain an independent approval and protected fast-forward merge.
4. Cut the next unused immutable `redline-central-v4.1.0-jain.N` tag.
5. Bind the exact tag commit and release-tree SHA-256 in the Redline authority.

## Integrity and provenance

The artifact lane records the locked dependency digest, source commit, source
tree, and package SHA-256. The security lane produces a dependency audit,
secret scan, and SPDX SBOM. Accepted release evidence must be regenerated from
the merged commit; review-branch output is readiness evidence only.

## Rollback

Rollback selects the previous accepted immutable `.jain.N` tag and its bound
tree checksum. It never moves or deletes a tag. Database data rollback is a
separate operator action and requires a verified backup; code rollback must not
silently downgrade or rewrite the persisted Redline format.

Local release proof:

```sh
rtk bash scripts/ci-local.sh required
```

The artifact lane builds both Rust binaries, packages them with the Docker and
release documentation, and records SHA-256 evidence under
`target/artifact-support/`. Generated evidence is not source authority; the
accepted tag commit and tree checksum are bound by the Redline control plane.
