# Monorepo → Split Port-Delta (2026-07-08)

Authoritative record of where recent `jain_small` (monorepo) work stands relative
to the 19-repo `jain-split` family, as of **2026-07-08**.

- **Monorepo source of truth:** `jain_small`, branch `apex`, HEAD `cc27936`
  ("Use Chimera-only safetensor bundle") **plus uncommitted working-tree
  changes**.
- **Status of the monorepo:** **FROZEN** by jekko on **2026-07-08**,
  reference-only. No further apex development is expected; this delta is the
  closing reconciliation between the monorepo and the split family.
- **Method:** synthesis of the port program's ledger — *not* a re-derivation
  from a fresh diff. Each row records a decision already taken, not a new finding.

The recent `apex` surface reduces to four buckets: already ported before the
split, ported in this program on 2026-07-08, parked by explicit product
decision, and areas where the split has already moved ahead of the monorepo.

---

## 1. Ported pre-split (already committed in the split family)

Landed in the split before this program; no action required, listed for
completeness and traceability.

| Monorepo feature | What it is | Split status |
| --- | --- | --- |
| Canary active-purge | Phase-1 `canary_purge` drop of canary-referencing GP survivors + cockpit `CanaryPurge.tsx` dramatization | Ported (committed) |
| Hyperion v7 true-forward sweep | Real v7 transformer forward (`hyperion_v7/`, candle-gated) + `encode.sweep` cockpit side-channel | Ported (committed) |
| `reasoning.note` side-channel | Human-reasoning narration riding the `reasoning.note` event kind + `ReasoningLine.tsx` (off the governed Event contract) | Ported (committed) |
| Session sidebar | `GET /api/sessions` list/rename/delete + account/connectors scaffold | Ported (committed) |
| jboost naming boundary | User-facing "jboost" brand mapped only at UI/artifact/CLI edges via `naming::display_model_name`; internals stay "catboost" | Ported (committed) |
| Headerless CSV sniff | `--header auto` sniff for headerless CSV in `jain bench` | Ported (committed) |

---

## 2. Now ported (this program, 2026-07-08)

Work carried across in this wave.

| Monorepo feature | Split landing | Notes |
| --- | --- | --- |
| Row-stability inference + GPU memory governor | `jain-core` `8b9e8f5`, `jain-starforge` `538c789` | Sourced from the never-merged `neverhuman/greenlight-7.0.1` branch (see §4). Removes batch-relative inference ops (GP rank/zscore, fuser rank, Starforge classifier rank→median) for row-stable predictions; adds the `JAIN_GPU_MEM_MB` adaptive query-batch / CatBoost column-cap governor. |
| InventionLab UI expansion | `jain-web` (this wave) | Phase-6 math-invention feed-takeover UI surface brought forward. |

---

## 3. Parked by decision (not ported)

Rationale for the whole bucket: **the customer deliverable is a single image;
the cloud/multi-tenant tier is future work.** These are intentionally excluded
from the split family in this cut.

| Monorepo surface | What it is | Disposition |
| --- | --- | --- |
| Multi-tenant cloud backend | `cloud.rs` / `server.rs` + guest/queue endpoints + the `jain-worker` crate | Parked — cloud tier is future work |
| `jain web` CLI subcommand | The in-CLI web launcher | Parked — the split ships a **separate `jain-web` binary** via `jain-deploy` `Dockerfile.cloud-web` (`--entrypoint`), so the CLI subcommand is not carried |
| `jain-public` landing SPA | Public marketing/landing single-page app | Parked — single-image customer scope |

---

## 4. Split is ahead of the monorepo

Where the split family already carries work the frozen monorepo does not; no
back-port to `jain_small` is warranted (it is reference-only).

| Split-only capability | Note |
| --- | --- |
| `EVENT_SCHEMA_VERSION = 10` | Governed 3-language progress-event contract is a version ahead of apex |
| feat-math surrogate evaluator | Cheap ridge-surrogate genome ranking for phase-6 invention |
| Multi-dataset uploads | Split web accepts multiple datasets |
| `installer_auth` | Installer authentication path present only in the split |
| Cloud image family | Split-side cloud image lineage (beyond the single customer image) |

---

## 5. Notes on provenance

- **`jain_small` is FROZEN** (jekko, 2026-07-08) and is **reference-only** from
  this point. This document is the authoritative closing map of what did and did
  not cross into the split.
- **`neverhuman/greenlight-7.0.1`** (9 commits, **never merged to `apex`**) was
  the source of the **row-stability inference + GPU memory governor** work now
  ported in §2. The branch is otherwise **superseded**: its JBoost *full-rename*
  approach was replaced by the `naming.rs` display-boundary mapping (the jboost
  naming boundary already ported in §1), so nothing else from that branch is
  carried forward.
