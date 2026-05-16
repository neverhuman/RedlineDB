# Language Bad Behavior — Detector Terms

The jankurai audit greps product code for a small set of terms that
historically signal future-hostile drift: code that is half-deleted,
unowned, or pretending to be something it is not. This doc enumerates
the terms, explains what each one means, and records the local
conventions RedlineDB uses so contributors do not trip the detector
by accident.

## Detector terms

| Term          | Why it is hostile                                                                 | What to do instead                                                                 |
|---------------|------------------------------------------------------------------------------------|------------------------------------------------------------------------------------|
| `placeholder` | Hides an unimplemented path behind a name that compiles.                          | Implement it, or return a typed `Error::Unsupported { feature: ... }`.             |
| `temp`        | Owner-less scratch state that survives past its expiry.                           | Rename to a concrete noun: `scratch_buf`, `staging_dir`, `swap_file`.              |
| `legacy`      | Implies "old but kept around"; usually the code is already dead.                  | Delete it, or rename to a version-pinned noun (`v1_*`, `pre_phase11_*`).           |
| `compat`      | Hides which API surface is being preserved and for which consumer.                | Name the consumer: `sqlite3_api`, `cross_engine`, `c_abi`.                         |
| `fallback`    | Swallows real errors via `unwrap_or_default` and friends.                         | Use `?` and a typed `Result<T, E>`; if the value is truly absent, model it.        |
| `todo`        | An IOU that ages out and rots into silent acceptance.                              | Open an issue, link it: `// TODO(#123): ...`; or implement and delete the marker.  |
| `stub`        | A function that exists only to satisfy the type system, not the caller.            | Implement it, return a typed error, or delete the call site.                       |
| `old`         | Implies a `new` exists; if both stay, callers must guess which to use.             | Delete the `old` path; if both are intentional, rename both to specific nouns.     |
| `unused`      | Dead code blocks human readers and the compiler's reachability analysis.           | Delete it (the tests in `git` are the receipt).                                    |
| `stale`       | Marks a value as out-of-date without saying when it should be refreshed.           | Encode the freshness contract: `last_refreshed_at: Instant`, with explicit checks. |

## Local conventions

- `compat` is renamed to `sqlite3_api` in the FFI Rust layer; the C-ABI
  symbols keep their `sqlite3_*` names (the rename is module-internal
  and uses `pub use` re-exports to preserve external symbols). The
  bench harness's `compat` mode is renamed to `cross_engine` to match
  the actual code path.
- `legacy` is not used; version-pinned nouns (`v1`, `pre_phase11`) are
  preferred. When a phase introduces a v2 format, the v1 reader stays
  named `v1` until the on-disk format-floor drops it.
- `temp` is reserved for genuine RAII scratch (e.g. `tempfile::TempDir`
  binding names); module-level `temp_*` is not used.
- `fallback` is forbidden in product crates. If a path needs a default,
  it is named `default_*` and the default is documented at the type.
- `TODO` comments without a ticket reference are rejected by the audit.

Generated and ABI surfaces (where some of these terms are unavoidable
because they describe external contracts) are listed in
`agent/generated-zones.toml`. Per-file carve-outs live in
`docs/exceptions/` once those files are authored.

## web-security-and-repo-rot-detectors

The same detector engine catches a second class of issue rooted in
HTTP/web boilerplate (CORS holes, secret-in-source, SSRF sinks) and
repo rot (fake-versioned filenames, abandoned branches, expired exception
markers). RedlineDB ships no web frontend, so the web detectors fire
only on documentation; the repo-rot detectors govern bench TOMLs and
module headers under `crates/{bench,ffi,redlinedb}/`. The full
detector reference and the local exception schema are tracked in
`agent/audit-policy.toml`; mitigations land via Section B of the
repair plan.

## Rerunning the detector

```
jankurai audit . --mode advisory \
  --json agent/repo-score.json --md agent/repo-score.md
```

The audit's `future-hostile-dead-language-in-product-code` cap and
`repo-rot-bad-behavior` cap are the two it can lift; see
`docs/audit-rubric.md` for the full dimension map.
