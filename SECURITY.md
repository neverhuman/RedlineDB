# Security policy

RedlineDB is the front-door for the redline family. Report vulnerabilities against
the repository that owns the affected code:

- **Engine / storage / SQL / RQL / FFI** → [redline-core](https://github.com/neverhuman/redline-core)
- **Conformance / benchmark harness** → [redline-testing](https://github.com/neverhuman/redline-testing)
- **SQL console / observability backend** → [redline-web](https://github.com/neverhuman/redline-web)
- **This hub (installer, release pipeline, links)** → this repository

## Reporting

Open a **private security advisory** on the owning repository (GitHub →
Security → Report a vulnerability), or email the maintainers. Please include a
reproduction and the affected version/commit. Do not open a public issue for an
unfixed vulnerability.

## Scope notes

- The installer (`install.sh`) downloads release artifacts over HTTPS from this
  repo's GitHub Releases. Verify checksums published with each release.
- Release binaries are built from a **pinned** `redline-core` tag by
  [`.github/workflows/release.yml`](.github/workflows/release.yml); the provenance is
  the tag recorded in the release notes.
