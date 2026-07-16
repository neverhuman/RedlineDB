# Jain CatBoost 8.0.1 authority identity reconciliation

- Recorded: `2026-07-16T08:13:00Z`
- Decision: **BLOCK immutable tag creation until protected authority reconciliation**
- Scope: read-only design evidence. This note did not change the authority manifest, a derived
  manifest/lock, source, index, Git ref, check, PR approval, merge, tag, or deployment state.

## Exact observed state

The sole authority is `jain-split-ops/repos.manifest.toml`, observed byte-unchanged in the held
canonical checkout at SHA-256
`dbec989b1b5d2b04a26ad920f6e5946c8233c53db98eea9938860a5d962fc977`.
Its `jain-catboost` entry currently declares:

```toml
product_version = "8.0.1"
tag_revision = 2
current_tag = "jain-catboost-v8.0.1-split.2"
release_commit = "PENDING"
release_checksum_sha256 = "PENDING"
required_check = "jain-catboost/required"
```

Live forge readback at the same boundary proves:

- protected `main` is `056a605dc60554ddcf40c6b5b0cb07d71d68eb96`;
- PR #8 branch is exact `182089fb4caad2c752ba9e5a5bd584adceab4e90`, tree
  `6a39984d52f32a11c96aef8df3418ad525c9348c`;
- existing immutable `jain-catboost-v8.0.1-split.0` points to protected main but its payload
  `VERSION` is the invalid old identity `jain-catboost-v8.0.0-split.2`;
- neither `jain-catboost-v8.0.1-split.1` nor `jain-catboost-v8.0.1-split.2` exists;
- the exact PR head consistently declares `jain-catboost-v8.0.1-split.1` in `VERSION`,
  `agent/standard-version.toml`, README, and changelog; and
- therefore `jain-catboost-v8.0.1-split.1` is the actual next-unused corrective identity.

The PR content review passes, but `jain-catboost/required`, vendor-backed native proof, and the
Rust LCOV coverage source remain absent. No merge/tag authority can be inferred from the successful
automatic proof check.

## Exact protected authority change required

Do not edit the held authority checkout now. After CatBoost exact-head required CI is green,
independent approval is recorded, and protected fast-forward merge makes the reviewed commit the
exact protected `main`, obtain the stopped-head authority handoff and open a normal branch directly
from current protected Split Ops `main`. In one reviewed authority change:

1. Reread CatBoost protected `main`, the open/merged PR, required check, proof, and tag namespace.
   Abort if the merged commit is not the independently reviewed head or if split.1 has become used.
2. Change only the CatBoost authority selection from `tag_revision = 2` / `current_tag =
   "jain-catboost-v8.0.1-split.2"` to `tag_revision = 1` / `current_tag =
   "jain-catboost-v8.0.1-split.1"`.
3. In the same authority change, replace the `PENDING` pair with the exact merged CatBoost-main
   commit and its `git archive --format=tar <commit>` SHA-256. If the current PR head merges
   unchanged, the expected pair is:

   ```toml
   release_commit = "182089fb4caad2c752ba9e5a5bd584adceab4e90"
   release_checksum_sha256 = "eb02574d43b31d5c44afa45982629a29834c082adf3d748f5a69c040daa2b043"
   ```

   These values are a read-only precomputation, not authority. Recompute them from protected
   merged main and fail closed on any mismatch.
4. Preserve `product_version = "8.0.1"`, `required_check = "jain-catboost/required"`, protection,
   remote, paths, wave, role, and every unrelated repository entry byte-for-byte.
5. Regenerate every required derived manifest/lock only through the sanctioned Split Ops
   generators, verify byte identity back to the authority, and include only those deterministic
   generated outputs required by the validators. Never hand-edit a mirror or lock.
6. Run exact-head authority validation and required CI, obtain an independent approval, and
   protected-fast-forward merge the authority PR. Do not relax protection or waive a check.

## Required tag ordering

Current `splitctl::validate_manifest_tag_request` compares the requested remote, tag, commit, and
archive checksum to the authority and rejects any difference. It therefore rejects both the
current split.1 request (authority says split.2) and any tag request while commit/checksum remain
`PENDING`.

Consequently, after the protected CatBoost merge, the exact authority selection and binding above
must protected-land **before** running `splitctl immutable-tag`. Only then may the release owner:

1. create or verify `jain-catboost-v8.0.1-split.1` at the exact authority-bound commit;
2. read the immutable tag back from the loopback forge twice;
3. verify the tag payload declares split.1 and its tree checksum equals authority; and
4. refresh mirrors/snapshots only from the protected authority.

Any release documentation that requires tag creation before authority binding conflicts with the
current validator and must not be used to bypass it. Resolve that control-plane ordering mismatch
through its own protected review; never create an unvalidated tag, move split.0, skip split.1, or
hand-edit the authority to manufacture a green result.
