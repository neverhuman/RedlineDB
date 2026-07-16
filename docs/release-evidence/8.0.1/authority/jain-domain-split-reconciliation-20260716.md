# Jain Domain 8.0.1 post-merge authority and tag reconciliation

- Recorded: `2026-07-16T08:30:00Z`
- Decision: **BLOCK immutable split.1 tag until the exact merged commit/checksum protected-land in authority**
- Scope: read-only design plus a non-apply fail-closed tag dry-run. This note did not mutate the
  authority manifest, a derived manifest/lock, Domain source/ref, a tag, check, PR, approval,
  protection, or deployment state.

## Exact observed state

The sole authority is `/home/ubuntu/jain-split/jain-split-ops/repos.manifest.toml`, observed in the
held canonical checkout at SHA-256
`dbec989b1b5d2b04a26ad920f6e5946c8233c53db98eea9938860a5d962fc977`.
Protected Split Ops `main` remains `88a673c6c0e829721eebf8265036c1b18f2ef139`; the held local
branch is `claude/v8-release-fixes-20260714@85f0c16a0cd1d8c3af068a987285c95b20825364`
with unrelated modified/untracked evidence and runner state. This note did not edit or stage it.

The authority's Domain entry currently declares:

```toml
product_version = "8.0.1"
tag_revision = 0
current_tag = "jain-domain-v8.0.1-split.0"
release_commit = "PENDING"
release_checksum_sha256 = "PENDING"
required_check = "jain-domain/required"
```

Live forge and canonical readback prove:

- PR #8 protected-merged at exact independently reviewed head
  `3ae83ce17bb57f5f8dd52f087991ff59bda5afbc`, tree
  `3ba0da9c975bfbf28e42bce819530d0d2e3a24c5`;
- forge `main`, canonical `HEAD`, local `main`, and `origin/main` all equal that exact SHA and the
  checkout is clean;
- automatic `jankurai/proof` check `38110b48-a0ae-4b2f-8556-ada796545d21` and exact-head
  `jain-domain/required` check `7dd3236a-c07e-4b83-8840-c7d849fd676b` succeed;
- independent approval, protected merge, and lifecycle receipts hash `7a7188568e1968be...`,
  `a1768e715a47b3f...`, and `d737141dc304695a...` respectively;
- the exact merged release archive SHA-256 is
  `63467819b78a8c196eca6045e1c618d2758a61f813676a694543cec1f4eb4134`;
- occupied immutable `jain-domain-v8.0.1-split.0` points to `07a6879b...`, whose authored payload
  is historical 8.0.0-path content; it must never move; and
- `jain-domain-v8.0.1-split.1` is absent and is the next-unused corrective identity.

## Machine-proven fail-closed blocker

A non-apply `splitctl immutable-tag` request for split.1 at the exact merged commit exited failure
before any ref operation because the requested remote/tag/commit/checksum differ from the
authority's split.0/PENDING selection. Its receipt is
`jain-domain-split1-immutable-tag-dryrun-20260716.json`, SHA-256
`15a0dcd70f640c03b166aa1774d70421132d6ed29c7d4b85d4cc4dbe476bcb14`.

Source inspection confirms `validate_manifest_tag_request` computes the reviewed commit and
release archive checksum, then requires the requested remote, tag, commit, and checksum to equal
the canonical authority before `create_or_verify_immutable_tag` runs. No raw-tag workaround is
authorized.

## Exact protected authority change required

After the authority owner provides a stopped clean handoff, create one ordinary branch directly
from current protected Split Ops `main`—never a Git worktree—and make one reviewed Domain-scoped
authority change:

```toml
product_version = "8.0.1"
tag_revision = 1
current_tag = "jain-domain-v8.0.1-split.1"
release_commit = "3ae83ce17bb57f5f8dd52f087991ff59bda5afbc"
release_checksum_sha256 = "63467819b78a8c196eca6045e1c618d2758a61f813676a694543cec1f4eb4134"
required_check = "jain-domain/required"
```

Before committing, reread protected Domain main/checks/tag namespace and recompute the archive
checksum. Abort on any drift or if split.1 has become occupied. Preserve the Domain remote, paths,
role, wave, protection, required-check name, fail-closed release metadata, and all unrelated repo
entries byte-for-byte. Regenerate derived outputs only through sanctioned generators, run exact-
head authority validation/required CI, obtain independent approval, and protected-fast-forward
merge the authority PR.

Only after that exact authority selection and binding protected-land may the release owner run
governed `splitctl immutable-tag`, read split.1 back from the loopback forge, verify payload and
archive checksum, and refresh mirrors/snapshots from protected authority. The current control-plane
ordering conflicts with prose that says tag-before-bind; resolve that workflow discrepancy through
protected control-plane review, never by moving split.0, manually creating a tag, weakening the
validator, or hand-editing a derived manifest.
