# Authority refresh — jain-web to split.5 (2026-07-28)

`jain-web` was bound at `jain-web-v10.0.0-split.4`. Landing `veox/jain-web#17`
(the Redline consumer pin to core jain.6) advanced protected `main` and cut
`jain-web-v10.0.0-split.5`, which left the authority row asserting a release
identity one tag behind the repository. This record accompanies the manifest
edit that corrects it.

Refreshing a bound row is the expected cost of landing on an already-bound
member, not an exception: the row must move whenever the member does, or
`release-preflight` is fail-closed against a member whose recorded identity no
longer exists as its head.

| Field | Value |
| --- | --- |
| member | `jain-web` |
| immutable tag | `jain-web-v10.0.0-split.5` |
| release commit | `80f061093ac5792bf1a4614cabc4eeec0167827c` |
| release tree | `c14dbb5d97c89a068ca35e5bfd6f4c1081aab8df` |
| archive sha256 | `fb2ee466fc4be13b2798683f03387bf44e6a9e7924bfb47d9532ae6a7eaa4ae0` |
| supersedes | `jain-web-v10.0.0-split.4` @ `fc41407255055316ca346f5c8d430a3933350a26` |

Derivation, all against the authenticated forge rather than local state: the
immutable tag resolves to the same commit as protected `main`; the tag is
lightweight, so `refs/tags/<tag>^{}` returns nothing and the tag object is the
commit itself; `release_tree` is `<commit>^{tree}`; `archive_sha256` is the
SHA-256 of `git archive --format=tar <commit>`, the derivation `splitctl`
applies in its own exactness check.

No route block is recorded here. This edit carries the identity of a landing
reviewed and merged by the jain-web lifecycle owners, not by the author of this
manifest change; claiming their identities as this record's route would
misrepresent who reviewed what. The verification procedure in
[`docs/testing.md`](../../testing.md) applies unchanged.
