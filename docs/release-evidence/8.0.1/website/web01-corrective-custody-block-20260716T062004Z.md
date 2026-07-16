# WEB-01 corrective implementation custody precheck

- Claim: `codex-mcp-v801-web01-corrective-custody-20260716T0619Z`
- Checked at: `2026-07-16T06:20:04Z`
- Release: Jain `8.0.1` candidate
- Decision: **BLOCKED BEFORE SOURCE MUTATION**
- Required policy: use only the sole canonical primary checkout; no worktree, clone, stash, reset,
  forced checkout, dirty overlay, or active-owner interference.

## Outcome

The actual WEB-01 corrective branch was **not created**. The only canonical `jain-web` checkout is
dirty, is not on protected main, is not forge-equal, and has no stopped-head handoff. An exact-head
Web CI process is also still active and references the canonical path. Switching this checkout or
layering the correction over its unrelated WIP would risk losing or commingling someone else's
work and would violate the repository's single-primary custody rule.

No checkout/index/ref/object/branch/source/forge/PR/check/host/route/deployment state was changed.
The only writes are this receipt and the append-only coordination result.

## Canonical checkout identity

| Field | Machine readback |
|---|---|
| path | `/home/ubuntu/jain-split/jain-web` |
| Git common dir | `.git` |
| registered worktrees | exactly one: the canonical primary |
| worktree-list SHA-256 | `75f3460342e656f78cd9b5cd441b07c52cae4a311e0d01a699d8b28f52cd3261` |
| branch | `codex/v8-web-jope-smartcluster-20260713` |
| local HEAD | `c8015162671843de1229625c63a49c4a69aaac60` |
| local tree | `815e79a3d3b78da269a77fd47e55dc21b944abdf` |
| HEAD author | `Claude-orch <noreply@anthropic.com>` |
| HEAD subject | `chore(cleanup): checkpoint working state (Claude-orch owner-directed); nothing discarded` |
| forge protected main | `13ea4968dde1286111dacf8cd0216fa71e931242` |
| protected-main tree | `ad0234862cacb9221c27346f8b570552fd5d74a8` |
| stale local `origin/main` | `6adcda48e5711dcf5934934236be09fcc959ff10` |

The new protected-main object is intentionally absent from the canonical checkout's object DB.
No fetch was performed because even a read-targeted fetch mutates repository refs/objects in the
held checkout. Live forge truth was read from `git ls-remote`, and the exact protected commit/tree
was read from the existing family bare mirror.

## Dirty-state custody identity

- tracked modified paths: **36**
- tracked binary diff: **536 insertions, 105 deletions**
- porcelain-v2 NUL-stream SHA-256:
  `49c9183789b40719e8bffad0f3823a686c11709d55c338c62e783b7f6d6e8ff4`
- full `git diff --binary` SHA-256:
  `d3894c7a17ffa5d5afbdad6ee8d31ebb848d297891c24807fc2d147cc20951a2`
- untracked path: `.bundles/orphan-web-f235a471-20260716.bundle`
- bundle SHA-256: `367ecb496b63f81e8b04a81cca5ac6c26d8fa43795bce661c86688aa75bd9cd0`
- `git bundle verify`: pass; complete history; bundled `HEAD` is
  `f235a471ff716c3d8bd5577f3f241d2ce07a09ed`

Modified paths, exactly as reported by porcelain:

1. `apps/web/src/App.test.tsx`
2. `apps/web/src/components/EncodingProgress.tsx`
3. `apps/web/src/fileTransfer.ts`
4. `apps/web/src/generated/progressEvent.ts`
5. `apps/web/src/lib/defaults.ts`
6. `apps/web/src/protocol.contract.test.ts`
7. `apps/web/src/protocol/progress.ts`
8. `apps/web/src/protocol/sessions.ts`
9. `apps/web/src/protocol/training.ts`
10. `apps/web/src/state.test.ts`
11. `apps/web/src/state/fold.ts`
12. `apps/web/src/state/types.ts`
13. `contracts/progress-event.schema.json`
14. `contracts/progress-events.jsonl`
15. `crates/feat-web/src/cluster/dispatch.rs`
16. `crates/feat-web/src/cluster/training.rs`
17. `crates/feat-web/src/profile/dataset.rs`
18. `crates/feat-web/src/profile/session.rs`
19. `crates/feat-web/src/protocol/events.rs`
20. `crates/feat-web/src/protocol/records.rs`
21. `crates/feat-web/src/protocol/training.rs`
22. `crates/feat-web/src/routes/actions.rs`
23. `crates/feat-web/src/routes/router.rs`
24. `crates/feat-web/src/routes/sessions.rs`
25. `crates/feat-web/src/routes/uploads.rs`
26. `crates/feat-web/src/runner/artifacts.rs`
27. `crates/feat-web/src/runner/completion.rs`
28. `crates/feat-web/src/runner/config.rs`
29. `crates/feat-web/src/runner/core.rs`
30. `crates/feat-web/src/runner/invention.rs`
31. `crates/feat-web/src/runner/runtime.rs`
32. `crates/feat-web/src/runner/worker.rs`
33. `crates/feat-web/src/store/collaboration.rs`
34. `crates/feat-web/src/store/schema.rs`
35. `crates/feat-web/src/store/sessions.rs`
36. `crates/feat-web/tests/api_http.rs`

None of the four intended WEB-01 surgical files is among the tracked dirty paths, but that does
not make a dirty overlay safe: branch switching/rebasing would still operate on a held checkout,
and the correction must be reviewed as a clean exact-main delta.

## Active process boundary

At precheck, PID `2548559` was alive:

```text
bash /home/ubuntu/jain-split/jain-split-ops/ops/ci/.claude-shci-snap.sh \
  jeryu jain-web 3144411e9f2e6d54077c31a11b2060b23a7c3ec1 \
  /home/ubuntu/jain-split/jain-web jain-web/required
```

- process cwd: `/home/ubuntu/jain-split`
- `/proc/2548559/cmdline` SHA-256:
  `62a6f3c744bbb4832201c93cf940bc023ffdb5ae5ac3febbad69514b8797a2c4`

This is the separately owned exact-head PR #28 CI run identified by the coordinator. It was not
signalled, inspected beyond process identity, or otherwise disturbed. Even if its product build
uses an independent sandbox, its live path reference is another reason not to reinterpret the
canonical checkout as handed off.

## Exact unblock condition

WEB-01 implementation may resume only after an explicit stopped-head handoff that:

1. assigns custody for all 36 tracked modifications and the verified untracked bundle;
2. leaves the sole canonical checkout clean, on the then-current protected `main`, and forge-equal;
3. confirms no source producer, branch switch, rebase, or process holds that checkout;
4. preserves the current PR #28 CI outcome and removes its standalone sandbox normally;
5. permits a new surgical branch from the freshly read protected main.

Once those predicates pass, the already-reviewed correction is bounded to the merged Web-control
files: exact byte matching in `command.rs`, no browser trimming in `live.rs`, strict client-frame
fields and small pre-JSON WS limits, plus focused unit/property/real-WS/browser negatives. It then
requires bounded local gates, a non-force push, and a protected PR. Approval, merge, tagging,
authority updates, heavy CI, and production mutation remain outside this claim.
