# Local release process

The authoritative RedlineDB release path is 100% local. It uses the Jeryu
forge, physical custody below `/home/ubuntu/jain-split`, and immutable
`redline-core-v<semver>-jain.<revision>` tags. GitHub, crates.io, downloads,
external checkouts, symlink staging, and registered Git worktrees are not
release inputs.

## Version identities

- Package/runtime SemVer is declared by every active crate. The compatible
  hardening release is `4.2.0`.
- `redlinedb::STORAGE_FORMAT_VERSION` is monotonic and independent of SemVer.
  Version 4.2.0 retains storage generation 1.
- Testing and control tools have independent SemVer; evidence binds their exact
  commits and binary hashes.

## Protected lifecycle

1. Begin from protected local-Jeryu `main` in the sole canonical checkout.
2. Run formatting, locked/offline unit and integration tests, deterministic
   isolation schedules, affected compatibility shards, security, the
   full-graph dependency review, and package gates. Every gate is required;
   none is advisory or soft-gated.
3. Run `redline-testing run --contract ... --mode release`. Evidence is valid
   only when the engine, runner, corpus, contract, dependency custody, oracle,
   and performance-baseline hashes still match.
4. Run `redlinectl offline-containment` against the exact Jeryu commit. Its
   automatically removed `git clone --no-local` has no remote during tests,
   receives only in-tree Cargo/oracle custody, and runs under
   `IPAddressDeny=any` with `cargo test --locked --offline`.
5. Publish the branch, required check, independent approval, and fast-forward
   merge through the reviewed local lifecycle. Never waive a consumer, weaken
   protection, force-push, or bypass review.
6. Read the next unused `-jain.N` suffix from Jeryu and create that immutable
   tag once. Existing tags never move.
7. Bind the merged commit and tree checksum through the protected Redline
   authority change. Produce fresh Jain and Jeryu consumer receipts, then let
   `redlinectl proof-refresh` write both lock copies transactionally.

Candidate metadata remains fail-closed (`status=candidate`, `formal_ga=false`,
push disabled, rollback `7.0.6`). Production activation requires separate
owner authorization.

## Compatibility and rollback

Release claims must name either `redline-sqlite-contract/v1` or
`redline-postgres-contract/v1`; unqualified full-product parity is forbidden.
SQLite file/C-API identity and PostgreSQL wire/server/extensions are outside
those contracts.

Every major release proves backward reads, any required migration, rejection
of unsupported future storage generations, and rollback behavior. Rollback is
a reviewed source/pin change to the prior known-good immutable tag. If corrected
bytes are needed, cut the next unused tag; never delete or move old evidence.
