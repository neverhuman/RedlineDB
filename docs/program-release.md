# Manifest-driven program releases

`splitctl program-release` validates future release programs alongside the existing family release
without changing the existing candidate manifest or command behavior.

The authority manifest is the only source for the release identity, governed paths, protected
legacy releases and manifests, protected tag fragments, canary topology, repository inventory,
design inputs, deployment inputs, and evidence groups. Rust contains format and safety semantics;
it contains no product release constant. Adding a release therefore adds reviewed data and evidence,
not a new code branch.

Validate an authority and the evidence index it names:

```text
cargo run --locked --quiet -- program-release validate \
  --authority authority/production-compute-storage.program-release.toml
```

Reduce the same immutable inputs to deterministic status:

```text
cargo run --locked --quiet -- program-release status \
  --authority authority/production-compute-storage.program-release.toml
```

`--record /absolute/path` is available only for `status`. The target must be a new file beneath the
authority's evidence root and below its root directory itself. Parent directories must already be
physical, canonical, and symlink-free. The writer uses exclusive mode-0600 creation, fsyncs the file
and parent, and rejects every protected-release component before opening the target.

Validation is fail closed:

- the authority, index, spec, custody inputs, baselines, and every claimed receipt are bounded
  regular non-symlink files with stable descriptor identity;
- governed paths are normalized, contain the declared release as a complete path component, and
  contain no protected release component;
- design-input copies match their family-root sources byte for byte, size for size, and digest for
  digest;
- protected file and tree-inventory baselines match the custody receipt;
- repository remotes and required checks derive from owner/name, and a tag requires a protected
  merged commit;
- evidence groups exactly match authority policy; required groups cannot be deferred, deferred
  groups cannot carry receipts, and passed/failed groups require digest-bound receipts in their
  declared roots;
- status is eligible only when every required group passes and signed deployment inputs are bound.

The current program remains blocked intentionally. A recorded `blocked` result is evidence of an
honest reducer, not release approval or activation authority.
