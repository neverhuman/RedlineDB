
## 2026-07-19 05:36 UTC — Claude (ship conductor) FINDING: the forge lifecycle is NOT credential-blocked — stale PR checks are the real gate

Read-only probe, no mutation. This contradicts a framing that several documents and entries still
carry ("`git ls-remote` returns 401", "the merge-token is absent", "everything is one credential
away"), so please re-check it rather than take it from me — the commands are below.

**The owner token drives the lifecycle interface today.** Using only the documented opaque
token-file argument against the committed binary:

```
jain-split-ops/target/debug/splitctl jeryu-local pr-list \
  --repo veox/<repo> --token-file ~/.jeryu/secrets/veox-owner-token
```

returns real typed PR JSON for every repository I sampled. Anonymous `git ls-remote` on `veox/*`
also succeeds now. The `merge-token` file at `~/.jeryu/secrets/merge-token` remains absent, but
nothing appears to require it: `veox-owner-token` is accepted.

**Open PRs family-wide (sampled 9 repositories):**

| repo | open PRs | state |
|---|---|---|
| `jain-web` | 2 | `#1` blocked, `#2` blocked |
| `jain-core` | 2 | `#2` blocked, `#3` blocked |
| `jain-deploy` | 2 | `#1` blocked, `#2` blocked |
| `jain-smartcluster` | 1 | `#1` blocked |
| `jain-split-ops` | 1 | `#2` blocked |
| `jain-cli` | 1 | `#1` blocked |
| `jain`, `jain-shard`, `jain-platform` | 0 | no PR has ever been opened for the current work |

**Why they are blocked — checks, not credentials.** `jain-web#1` head
`5c76e1120b6748cc3fb5aabfb038a54b8fed5e5f` carries `jankurai/proof` = **`failure`** and
`jeryu/autonomy` = **`action_required`**, both stamped `2026-07-14T04:26Z`. These are five-day-old
PRs against long-superseded heads whose checks failed and were never re-run. `mergeable_state:
blocked` is the protection layer correctly refusing a PR whose required check is red.

**So the actual gate on "rebase everything onto main and land it" is mechanical work, not a
secret:** for each repository — rebase the live branch onto protected `main`, `branch-push`,
`pr-open` (or repoint the stale PR), run `ops/ci/split-host-ci.sh` at the exact head so a **fresh
green** `<repo>/required` + `jankurai/proof` is posted, read the checks back, `protection-apply`,
have a **different identity** approve, fast-forward merge, then tag and bind. 27 of 39 checkouts are
currently ahead of `main` on a branch with no PR at all — that is the bulk of the remaining work,
and it is parallelizable one-writer-per-repo.

Two caveats I could not clear read-only: I did not exercise a write verb (`branch-push`, `pr-open`,
`pr-merge`) because every repository is currently claimed by another lane, so **write scope is
unproven** — a lane that holds a repo should confirm it on its own head before planning around it.
And `protection-readback` requires `--required-check`, so I have not inspected the protection
policy itself.

Whoever owns AUTH-001: this does not diminish the bootstrap work — it suggests the blocking
question may be narrower than "no usable credential". Worth one write-verb test on a repo you
already hold.

Frozen `RELEASE_V9.md` reverified byte-identical at SHA-256
`a0d27689889a3d19b2d11e86aa2ed0b73e6ca662d5949e554f51596b1f58bf2c`. No ref, PR, check, protection,
merge, tag, credential, host, registry, route, or production state was mutated by this probe.
— Claude (ship conductor)

## 2026-07-19 05:36 UTC — Claude (ship conductor) NOTE: `jain-jekko/` is a real repository family, NOT scratch — do not delete

Flagging because prior cleanup lists (including my own plan) named `jain-jekko/` as a deletion
candidate purely on its size and its absence from the manifest. **That is wrong.** It contains
**eleven clean Git repositories** — `jekko`, `jekko-agent`, `jekko-core`, `jekko-deploy`,
`jekko-jailgun`, `jekko-jnoccio`, `jekko-mcp`, `jekko-memory`, `jekko-search`, `jekko-web`,
`jekko-zyal` — each with a real forge remote under `.../git/jeryu/jekko-*.git`, and 1373 tracked
Rust source files. Ten are clean; `jekko-web` has 4 dirty paths (someone's uncommitted work — leave
it alone). **I have not touched it and no one should delete it.**

Of its 29 GB, **28 GB is regenerable `target/` build cache** across those eleven repositories. If
the owner wants the space back, `cargo clean` (or removing just the `target/` directories) reclaims
~28 GB with zero source loss and no forge interaction. I am not doing that unilaterally: it is a
different family, none of it is mine, and a build may be warm. Say the word and it takes a minute.
Note the family is on the legacy `jeryu` remote namespace, which the `veox` ruling did not cover —
that is an open ownership question for the owner, not something to fix by hand.
— Claude (ship conductor)
