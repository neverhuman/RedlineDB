# Manifest-driven program releases

`splitctl program-release` validates future release programs alongside the existing family release
without changing the existing candidate manifest or command behavior.

The authority manifest is the only source for the release identity and target claim, governed
paths, protected legacy releases and manifests, protected tag fragments, canary topology, repository inventory,
design inputs, deployment inputs, and evidence groups. Rust contains format and safety semantics;
it contains no product release constant. Adding a release therefore adds reviewed data and evidence,
not a new code branch.

Validate an authority and the evidence index it names:

```text
cargo run --locked --quiet -- program-release validate \
  --authority authority/production-compute-storage.program-release.toml
```

Validate every discovered authority without assuming a single program:

```text
cargo run --locked --quiet -- program-release validate-all --authority-dir authority
```

Reduce the same immutable inputs to deterministic status:

```text
cargo run --locked --quiet -- program-release status \
  --authority authority/production-compute-storage.program-release.toml
```

`--record /absolute/path` is available only for `status`. The target must exactly match the new file
declared by `paths.status_record` beneath the authority's evidence root. Parent directories must
already be physical, canonical, and symlink-free. The writer uses Linux descriptor-relative
`openat2` confinement, an exclusive mode-0600 temporary file, exact content read-back, atomic
no-replace publication, and file/parent fsync. It refuses to run
when the kernel cannot enforce `BENEATH`, no-symlink, no-magic-link, and no-cross-device resolution.

Validation is fail closed:

- the authority, index, spec, custody inputs, baselines, and every claimed receipt are bounded
  regular non-symlink files with stable descriptor identity;
- confined directories must be owned by the validating UID and cannot be world-writable. The local
  control plane treats its checkout owner and deployment group as one trusted administrative
  boundary; this is not a multi-tenant file-authenticity boundary;
- governed paths are normalized, contain the declared release as a complete path component, and
  contain no protected release component;
- the tracked design-input bundle matches the external intake observations recorded in custody; the
  standalone validator does not pretend it re-read mutable family-root sources, so live-source and
  intake-attestation claims remain false and custody remains pending;
- the exact authority-declared protected-surface set matches descriptor-read file and structured
  tree-inventory baselines;
- repository remotes and required checks derive from owner/name; local prototypes bind an exact
  branch and head. Review-pending and protected-merged states are rejected until typed signed proof
  and forge validators exist;
- every stable SPEC requirement is routed exactly once by policy;
- opaque evidence can never promote a group. Every required group remains `pending` until its
  closed typed signed validator exists; deployment binding, GA, activation, and Critical likewise
  fail closed while their signed validators are unavailable;
- reduced status binds authority, SPEC, custody, evidence-index, receipt-set, and repository-set
  digests into one deterministic decision-input digest.

Canary status, durability, and the human-readable program claim are explicitly target fields. They
describe the intended qualification outcome, never an achieved state; only the reduced decision can
state eligibility, and it remains blocked while any governed validator or deployment binding is absent.

The current program remains blocked intentionally. A recorded `blocked` result is evidence of an
honest reducer, not release approval or activation authority.

The exact-SHA control-plane contract lane passes both `--skip-remotes` and
`--skip-program-checkouts` because its automatically removed standalone checkout contains no sibling
product repositories. This explicit portability omission does not prove repository lifecycle state;
the ordinary family validator performs the clean checkout, branch, head, remote, and Cargo-source
census. Git evidence remains pending until that census is represented by a typed signed validator.
