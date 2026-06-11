# Lifecycle & deprecation plan

This front door (`RedlineDB`) is the stable public identity of the project. The rest of
the family is being consolidated around it. This document records the **gated** moves —
what changes, what blocks each change, and how it is verified — so nothing is retired
before it is provably safe.

> Status: **planned, not executed.** Nothing below has been removed yet. Each item ships
> only after its verification checklist passes and the whole family is green.

## The family today

| Repo | Public slug | Status |
|---|---|---|
| **RedlineDB** (this) | `neverhuman/RedlineDB` | **canonical front door** — stable |
| redline-core | `neverhuman/redline-core` | active — the engine |
| redline-web | `neverhuman/redline-web` | active — console + observability |
| redline-testing | `neverhuman/redline-testing` | **to be consolidated** (see below) |

## 1. Retire the pre-split `~/redlineDB` checkout

The old `~/redlineDB` working copy was the original monolith, later repurposed as a thin
hub. Its useful content (README, installer, CI lanes, jankurai config, docs, typed
exception surface) has been carried forward into this repo — `redline-split/redline` —
with a clean history. The public GitHub slug **`neverhuman/RedlineDB` is unchanged**, so
no install URL, release link, or bookmark breaks.

- **What is retired:** the old local `~/redlineDB` checkout and any pre-split monolith
  branches/tags that are not part of this front door's `main`.
- **What stays:** the `neverhuman/RedlineDB` public repo and its URL; this repo becomes
  its canonical source on the jeryu forge.
- **Blocked until:** this repo is verified healthy (jankurai ≥ 85, 0 caps, 0 hard
  findings) and mirrors cleanly to `neverhuman/RedlineDB`.
- **Verification:** `bash ops/ci/pr-ci.sh` green here; one successful green-merge mirror
  to `neverhuman/RedlineDB`; then the old checkout/branches may be archived.
- **Rollback:** the old branches are archived (not deleted) until two clean release
  cycles pass; un-archive to restore.

## 2. Consolidate `redline-testing`

`redline-testing` is the conformance + benchmark harness. Its **durable asset is the
conformance corpus** (the SQLite-parity, RQL, and beyond-SQLite case sets and the signed
evidence bundle), **not** the runner binary. The runner becomes redundant once an
equivalent multi-engine tester is available, but the corpus must outlive it.

The hard constraint: `redline-core`'s official parity proof lane currently consumes
`redline-testing`'s **published release artifact** as its *sole* official evidence
source. Deprecating `redline-testing` therefore requires that evidence path to be
re-sourced first.

- **Blocked until all of:**
  1. The conformance corpus has a permanent home (its own retained release stream, or
     vendored into the engine's proof lane) — documented here with the exact location.
  2. `redline-core`'s parity proof lane is re-pointed at that home and is **green from
     the new source for N ≥ 3 consecutive runs**.
  3. `redline-web`'s CI regression corpus (which references the same cases) is re-pointed
     and green.
- **Then:** the `redline-testing` repo is marked deprecated (archived, read-only), its
  README pointed at the new corpus home; `family.json` / `FAMILY.md` / this README's
  family table are updated together (per the pointer-sync rule in `AGENTS.md`).
- **Verification checklist:**
  - [ ] Corpus home chosen and documented (location + retention policy).
  - [ ] `redline-core` parity lane green from the new source, 3 consecutive runs.
  - [ ] `redline-web` regression corpus re-pointed and green.
  - [ ] No remaining `neverhuman/redline-testing` references in any active CI lane.
  - [ ] Family pointers (`family.json`, `FAMILY.md`, `README.md`) updated in one change.
- **Rollback:** `redline-testing` is archived, not deleted; un-archive and revert the
  proof-lane re-point to restore the old evidence path.

## Gating principle

Nothing is archived until the family is verified healthy — all active repos green on
jankurai and CI — and the relevant checklist above is fully ticked. Archive (reversible),
never delete, until the replacement has proven itself across multiple cycles.
