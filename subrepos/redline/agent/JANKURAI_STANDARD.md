# Redline hub Jankurai standard

This repository is a shell-and-documentation hub. Engine code, Cargo manifests,
database ownership, runtime APIs, and web product code belong to the other
Redline repositories. A hub change must preserve that boundary and pass
`just check` plus `bash ops/ci/jankurai-audit.sh` from a clean snapshot.

Generated audit state belongs under `.jankurai/` or `target/jankurai/`; it is
evidence, not product truth. Rust-only checks are `not_applicable` only when
both `Cargo.toml` and `Cargo.lock` are absent. A partial dependency graph is a
hard failure.

Every failed lane must retain an agent-friendly receipt with:

- `purpose`: the invariant being proved;
- `reason`: the observed failure, including the original exit code;
- `common fixes`: bounded repairs that do not rewrite history;
- `repair_hint`: the owning file or repository;
- `rerun command`: the exact command that reproduces the lane.

Never replace a failure with a pass marker. Never move an immutable tag, edit a
family lock manually, force-push `main`, or run a production promotion from the
hub. Use Jeryu review and the Redline control plane for lifecycle operations.
