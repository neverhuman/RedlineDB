RedlineDB now includes the engine, conformance runner, Rust clients, web console,
release tooling and historical documentation in one ordinary GitHub checkout.

Core archives contain the CLI, server, native libraries and headers. Optional web
and testing archives are separate. The web binary embeds its frontend. Packages
support Linux x86_64/ARM64 (glibc 2.35+) and macOS Intel/Apple Silicon (macOS 15+).
Checksums, dependency notices, SBOMs and parent-commit provenance accompany builds.

The binary installer is noninteractive, defaults to ~/.local, accepts VERSION and
PREFIX, and never replaces sqlite3. Source builds use portable compiler defaults.
The preserved WAL/backup development branch is not included in this release.

macOS native libraries use a relative load identity and are signed after staging.
Native package checks link static and dynamic C consumers against the extracted
headers/libraries and run them after moving the installation directory.

Conformance reports retain declared skips and validate warmups for every executed
case. Required parity CI also generates a report from the complete evidence.
