# SUPER MERGER — Claude engineering specification

**Program:** One product — Jain — from the jain, jeryu, and jekko families
**Author lane:** Claude, root documentation lane (owner-directed 2026-07-17; parallel to `SUPER_MERGER_CODEX.md`, which is preserved byte-identical — sha256 `81fbaab3d52cace84284d087d65b2b5b41693d34a3520e844b2f2669abb347df`)
**Status:** Engineering specification; implementation requires separately claimed, reviewed lanes
**Target:** A future major release after Jain 8.0.1
**Character:** Where the Codex document is the normative *program* spec (MUST/SHALL contract language), this document is the *engineering* spec: it names the exact crates, files, protocols, and mechanisms that become the merged product, what is reused verbatim, what is amended, what is built new, and what dies. Every reuse claim in this file was verified against the trees on 2026-07-17 by a 14-agent exploration and design program. Section 15 reconciles this document against the Codex ADRs (SM-001..SM-015) so the two specs can be compared decision-by-decision.

---

## 0. Decision Zero — shape, name, firewall

### 0.1 The shape numbers

The entire merger compresses to seven numbers:

- **2 binaries** — `jainhub` (the hub) and `jainnode` (the worker). Nothing else ships as a daemon.
- **1 protocol crate** — `jain-fabric`: every typed workload, lease, fence, grant, stripe manifest, and receipt in one versioned, fail-closed registry.
- **1 ledger** — every execution anywhere is a row in the hub's work ledger. No row, no execution.
- **1 SPA** — one Vite/React/TypeScript app with **3 lenses** (HOME / CODE / KNOWLEDGE) over a persistent left pane + chat spine.
- **1 CAS** — one content-addressed encrypted artifact namespace for datasets, models, checkpoints, knowledge, build cache, and git bulk objects.
- **1 drill calendar** — every milestone exits through a drill that then becomes a permanent scheduled job.
- **~20 small repos** — not 48, not a monolith; see the small-repo doctrine (§10).

### 0.2 Name

**The product is Jain.** This is settled by existing owner posture, not preference: the production images are `veox/jain:*`, the public endpoints are `www.neverhuman.org/try` and `downloads.neverhuman.org/jain`, the owner demo entrypoint is `jain.sh`, and the production host is AtomicSoul. The design panel's alternative (JOVE) is recorded in Appendix C as rejected.

Sub-identities dissolve into internals:

- **jeryu** — the forge engine inside the hub. Never again a separately branded product surface.
- **ZYAL** — the name of the extracted headless reasoning service (per the Codex spec's terminology, adopted). **Jekko is a lineage, not a surface**: its TUI, web app, and chat server do not survive as products; its engines do.
- **jankurai** — keeps its name and badge. It is the product's conscience and the one internal name users see (`jankurai/proof`, the score badge).
- **jnoccio** — the internal model gateway name; never user-facing.
- **InnovationZero** — the dataset→code invention method (the Move42 paper: "InnovationZero Learns to Invent Executable Algorithms — A Verifier-Driven Foundation Model from Data to Code", served by `jain-web-move42/`) and the name of the entry tier experience (§1.2, §9).
- **SCQ / SmartCluster** — the scheduler spine's internal name (sole `[[infrastructure_repo]]` in the authority manifest).

### 0.3 Firewall from 8.0.1 (owner-locked)

Identical in force to the Codex spec's firewall, restated because it binds this document too:

- This specification MUST NOT change, delay, reinterpret, or become an exit gate for the 8.0.1 candidate.
- It cannot touch candidate metadata (`status=candidate`, `formal_ga=false`, `sagemaker=N/A`, `ATOMICSOUL_PUSH=0`, rollback target `7.0.6`), branch protection, immutable tags, release evidence, Redline locks/proofs, or the reviewed lifecycle.
- It authorizes **no** implementation lane by itself. Every implementation lane requires its own `UPGRADE_CHAT.md` claim, repository-owner rules, reviewed no-bypass lifecycle, and exact-head proof.
- Future-program evidence lives in its own namespace, never under `docs/release-evidence/8.0.1/`.

### 0.4 Owner decisions of 2026-07-17 (recorded)

Three forks between this document's design program and the Codex spec were put to the owner at plan review and settled:

| # | Fork | Owner decision |
|---|------|----------------|
| OD-1 | File collision between the two specs | **Keep both.** Codex's document preserved byte-identical as `SUPER_MERGER_CODEX.md`; this document stands at `SUPER_MERGER_CLAUDE.md`. |
| OD-2 | Repo consolidation depth (Codex: service merger, repos stay; panel: 48→12 monorepo-ward) | **Small-repo doctrine.** "With AI we want small code, and small repos, unless the repos are so small they are causing issues (e.g. closing us down with releases)." Consolidation is friction-driven, not aesthetic. See §10. |
| OD-3 | Control-plane HA (Codex: OpenRaft command journal; panel: epoch fencing) | **Epoch fencing v1** — designated follower, fleet-witnessed monotonic hub-epoch, owner-invoked promotion. Raft/OpenRaft is the explicit v2 upgrade path once ≥3 controller-eligible nodes are standard. See §7 and SM-011/012 disposition (§15). |

### 0.5 What each family contributes (one paragraph each)

**jain** contributes the product: the chat-first cockpit (`jain-web/apps/web` — Sidebar/Feed/Composer, slash commands, seq-dedup WebSocket in `useSessionSocket.ts`, per-session git via gix), the AutoML engine (`jain-core/crates/feat-core`: GP synthesis → importance epochs → PBIL selection, hyperion 128-column sweeps, invention GP, jope_embed with a frozen Python parity oracle at ~1e-5), the model backends (catboost/lightgbm/xgboost FFI + pure-Rust jable), GPU kernels (`jain-battle-gpu` with the StrictGpuSession known-answer gate), candle inference (`jain-starforge`), the guest tier (`jain-web-control` 10-slot lease edge + `guestd` durable WAL lease authority), and above all **SCQ** (`jain-smartcluster`): the durable scheduler with the owner-locked preemption law this whole product is built on.

**jeryu** contributes the governance engine: the local-first forge (one fused binary serving SPA + `/api/v1` + GitHub-compatible `/api/v3` + git smart-HTTP on `127.0.0.1:8787`), protected compare-and-swap refs (`jeryu-gitd/src/refs.rs`, `protection.rs`, a real server-side pre-receive guard in `hooks.rs`), the runner fabric (`jeryu-ci-runner`: TrustTier T0–T5, RunnerClass selection, the fail-closed `jeryu-sandbox-linux` unsafe island, workcells and warm pools), **live agent control** (`jeryu-agentbridge`: PTY driving of Claude/Codex/Jekko CLIs with SendInput/InjectPrompt/Interrupt/Terminate/RaiseBudget), codegraph intelligence, the CAS build cache with poisoning defenses, the richest SPA shell (`jeryu-web`: AppShell, cmdk palette, vim-chord keys, Monaco, xterm), and the identity spine (`jeryu-core/src/model/auth.rs` on RedlineDB).

**jekko** contributes the mind: the multi-provider agent runtime (`jekko-provider` streaming adapters + key pools, `jekko-runtime` agent loop), the R&D ledger (`jekko-store` daemon subtree: run/task/task_pass/iteration + reasoning memory capsules + forever{findings,concepts,regression} tables), **agent-search** (16 providers, provenance store with content-hash dedupe, query router, safety layer), **cogcore** (deterministic append-only cognitive memory: FSRS spaced repetition, Hebbian association, consolidation into SynthesizedLessons, 12-axis benchmark), the ZYAL runbook stack (zyalc compiler, zyal-supervisor, evidence receipts), jnoccio-fusion (the self-learning OpenAI-compatible gateway), and jailgun (the browser-driving air-gapped sandbox with its MCP tool server).

**Shared substrate:** RedlineDB (`redline-core` is byte-identical between `jain-redline/` and `jeryu-redline/` — the proof machinery already demonstrated it; §10 single-homes it) and jankurai (external auditor, pinned 1.6.11 via `jeryu-tool/tool-manifest.toml`, 48 HLT rule families, guard/proofbind/proofmark primitives — §8).

---

## 1. The object model — five nouns

Everything the user touches is one of five nouns. They are Rust types in `jain-fabric`; TypeScript is generated (never hand-edited — same rule as Codex SM-015, adopted).

### 1.1 Project — the only top-level object

```rust
Project {
  id, name, created_at, archived_at: Option<_>,
  repo: ForgeRepoRef,            // EVERY project has exactly one forge repo from birth
  data_class_default: DataClass, // Private | Project | Shared
  durability_default: DurabilityProfile,  // §6
  collaborators: Vec<Grant>,     // §2.4 — grant bundles, not a membership table
}
```

- **Every project is a real forge repo from day one.** A dataset upload creates `zero/<name>`: a hidden micro-repository whose history holds the dataset manifest (CAS pointer, not bytes), `/features`, `/models`, `PROJECT.toml`. Protection rules, jankurai scores, and PRs all already work the moment the project "grows up" — because it was never a different kind of thing. Source mechanics: jain-web's per-session gix repo (checkpoint/revert) is *promoted* to a forge repo; `jekko-store/src/project.rs` supplies the project/workspace schema seed (workspace dies, project survives).
- **The ambition ladder is derived, never stored.** A project with one branch, no PRs, and a dataset-typed root renders as a Zero card (dense, dataset-first, chat-zoomed). The moment a second session opens a branch or a PR exists, the same row renders as a full estate. InnovationZero → core-IP → JARVIS is not a mode switch or an upgrade wizard; it is how much of the surface the project's own contents have unlocked. This single decision deletes the "two products" problem at the schema level.
- **Two ingest doors, both first-class:** upload a dataset (→ `zero/<name>` micro-repo) **or import an existing repository** — `jeryu-mirror`'s import/replay engine (exists, unwired) is connected to project creation. "Everyone uploads data, code, whatever" requires both doors; the Codex spec's quick-upload flow (§3.8 there) is adopted for the dataset door.

### 1.2 Session — the universal driver

One session type, whether it chats, trains, searches, or drives a live-coding cell.

```rust
Session {
  id, project_id, branch: Option<BranchRef>,  // agent cells lease a branch; chat rides trunk
  transcript: Vec<Event>,   // seq-ordered; WS push + REST backfill (jain-web model, kept)
  grants: Vec<Grant>,       // dynamic per-path R&D data grants surface here (§8.4)
  checkpoints: Vec<CommitRef>,  // jain checkpoint/revert, now git-native on the forge repo
}
```

Merges three session models that already exist: jain-web sessions/events/artifacts (RedlineDB store, owner-scoped, Zod EventSchema, seq-dedup reconnect — `feat-web/src/store.rs`, `useSessionSocket.ts`), jekko-store `session{message,part,permission}`, and jeryu agent sessions (namespaced branch + hardened container + host-mediated publish — `jeryu-api/src/web/sessions.rs`). Slash verbs are the union: jain's (`/approve /start /next /stop /predict /files /upload /repo /checkpoint /history /revert /set /target /effort`) plus forge verbs (`/pr /merge /audit`) plus knowledge verbs (`/search /boil /grant`).

### 1.3 Run — one execution concept

```rust
Run {
  id, project_id, session_id: Option<_>,
  lane: RunLane,           // ci | train | search | agent | adhoc — a UI LABEL only (§4.3)
  spec: WorkUnitV1,        // §4.2 — the UX face and the kernel face are the same row
  state: RunState, tier: PriorityClass,
  lease: Option<LeaseRef>, receipts: Vec<ReceiptRef>,
}
```

Every row in every list that means "the machines are doing something" is a Run with identical lifecycle chips (`queued → leased → running → ckpt → done | killed | preempted`) whether it came from CI, a hyperion sweep, an agent-search fan-out, live coding, production training, or storage repair. The user learns one vocabulary, and preemption is legible everywhere ("production killed my batch job; requeued from checkpoint e11"). Codex's Run/attempt split (stable Run identity, per-attempt lease/fence/logs) is adopted verbatim — it matches SCQ's existing attempt model.

`RunLane` is a **UI label** with values the user recognizes (`ci | train | search | agent | adhoc`); it maps **many-to-one** onto the node's four *execution* lanes (`ci | prod | rnd | agent`, §4.3). For example, a `train` or `search` Run runs in the node's `prod` or `rnd` execution lane depending on tier; `adhoc` maps to `rnd`. The UI label answers "what is this for"; the execution lane answers "how does it get scheduled and preempted." They are deliberately different vocabularies for two different audiences.

### 1.4 Artifact and KnowledgeItem — one CAS, typed kinds

```rust
Artifact      { digest, kind: Dataset|ModelBundle|Report|FeatureSet|Predictions|Checkpoint|Sbom|Receipt,
                run_id, size, durability: DurabilityProfile }
KnowledgeItem { digest, kind: Source|Paper|Lesson|Concept|LeaderboardEntry|Finding,
                provenance: ProvenanceRef, project_refs: Vec<_>, fsrs: Option<FsrsState>, data_class: DataClass }
```

Artifact unifies jain session artifacts + SCQ v2's encrypted artifact store + jeryu-cache's build CAS into one content-addressed namespace (§6). KnowledgeItem unifies agent-search ProvenanceStore rows, cogcore concepts/SynthesizedLessons (with FSRS due-state), and jain leaderboard entries (GP winners, hyperion cells). Knowledge is project-scoped; the `Shared` data class is what makes the admin's cross-project rollup real (§2.5) — "admin benefits from ALL shared data/tooling/knowledge/telemetry" becomes a storage property, not per-endpoint logic.

### 1.5 AttentionItem — derived, never stored

A materialized view computed from signals that already exist, not a table anyone writes:

```
AttentionItem { severity, kind: release|audit|split|tool|rnd-review|pr|node|capacity, project_id, verb }
```

Sources (all verified): splitctl `release-preflight` state; `.jankurai/repo-score.json` (score < 85 or hard findings > 0); `jeryu-tool-finder` duplicate mass + repo LOC (split candidates); `jeryu-tool` pin drift; completed Runs with unreviewed artifacts; `autonomy_bridge` evidence verdicts (record-only, surfaced not acted); PR approval state; SCQ node quarantine; guest-slot pressure. This feeds the HOME lens (§3.2) and is the product's heartbeat. A warning with no actionable verb is telemetry, not an AttentionItem (Codex's rule, adopted).

---

## 2. Identity, tenancy, sharing, guests

### 2.1 One identity spine

`jeryu-core/crates/jeryu-core/src/model/auth.rs` is the spine — it already has durable `UserAccount { login, password_hash, role: Admin|User, active }`, PATs, SSH keys, `RepoAccessGrant`, and `CollaborationInvitation` on RedlineDB, served by `jeryu-api/src/web/auth.rs` (session cookie, CSRF, middleware). **Deleted as identity sources:** jain-web's owner access-token and `JAIN_WEB_NO_AUTH` (jain surfaces become forge-auth clients), jekko's per-user filesystem key pools (`~/.jekko/users/<user>/` — key custody moves server-side, §8.3), and the `jeryu-enterprise` RBAC/SSO/tenancy scaffolds (archived; the `AuthorizationDecision` shape is absorbed).

### 2.2 Principal/Grant — small on purpose

```rust
enum PrincipalKind { Admin, Member, Guest { slot }, Node { pubkey: Ed25519 }, Runner { pubkey: Ed25519 } }
enum Scope  { Repo{..}, Project{id}, Data{class, selector}, Compute{tier, budget}, Telemetry }
enum Level  { Read, Write, Approve, Operate, Admin }
struct Grant { principal, scope, level, expires_at: Option<_>, granted_by, project: Option<_> }
```

Five levels × five scope kinds replaces jeryu-web's ~28 fine-grained UI permissions (they become derived views: `pr.approve` = `Approve` on `Repo`), jeryu-enterprise RBAC, jain session collaborators, and jekko key pools. Two tables, one `authorize(principal, scope, level) -> Decision` with **denial receipts**. Deliberately not an IAM: no groups, no policy documents, no conditions beyond expiry. Any deny wins; a lagging decision path fails closed for mutations (Codex §6.1 semantics, adopted).

Nodes and runners are principals with ed25519 keys — the **same enrollment trust root** signs artifact encryption, lease fencing, and jankurai proof receipts (§8.2). One PKI, not three.

### 2.3 Admin

`UserRole::Admin` maps to `PrincipalKind::Admin`. Admin can: create/disable Members (existing `AdminSettingsPage.tsx` flows), set the floating-guest count (a runtime parameter backed by `appliance.toml`, §11), enroll/fence/revoke nodes, and hold an implicit `Grant(Telemetry, Read)` over everything labeled `Shared`. **Admin does NOT get implicit access to Private data** — that is the standing isolation-test invariant. Private-content access is break-glass only: a specific project + scope, recorded human reason, strong re-auth, ≤1 hour expiry, immutable trail, owner notification, automatic post-expiry denial (Codex §6.4, adopted verbatim — it is correct and complete).

### 2.4 Joint projects — grant bundles, not a subsystem

A joint project is a materialized Grant bundle built from two assets that already exist: jain-web's session collaborators routes and jeryu's `CollaborationInvitation`. Accepting an invitation materializes: `Grant(Repo(project repos), Write)` + `Grant(Data(Project-class), Read|Write)` + `Grant(Compute{tier: Interactive|Batch, budget: shared})`. The shared compute budget is a **single ledger row the scheduler debits** for any member's R&D jobs — collaborators genuinely share ad-hoc R&D compute, code access, and data. This is the joint-projects-on-production requirement with zero new subsystems. Enablement on AtomicSoul is gated by the tenancy isolation matrix going green on a staging appliance (§12).

### 2.5 Data classes and the telemetry rollup

Three labels attached at write time to every dataset, artifact, knowledge item, and receipt: **Private** (owning principal), **Project** (project members), **Shared** (all Members + admin rollup). Defaults: job/tooling/telemetry receipts = Shared; datasets, models, knowledge findings = Private until granted into a project. One receipts table (`receipt(id, principal, project?, kind, subject_digest, data_class, payload_digest, ts)`); every existing receipt producer (SCQ job receipts, ZYAL evidence receipts, jankurai proofs, preemption KPI receipts, cache receipts) gains one POST. Admin dashboards are read-models over `WHERE data_class='shared'` — **by construction, never bypass**.

### 2.6 Floating guests

`guestd` (`jain-deploy/crates/guestd`: WAL-durable leases, restart-monotonic epochs, deadline queue) survives as the lease authority, re-founded as a **hub library** — the standalone `guestd` and `jain-web-control` binaries retire (§13), their semantics survive:

1. **Principal binding.** A lease records `held_by: Principal`. Anonymous visitors get an ephemeral `Guest(slot)` principal; logged-in Members bind the lease to their Member principal.
2. **Two-class priority — "users get preference."** When capacity is exhausted and a Member requests a profile, the hub reclaims the anonymous lease closest to its idle deadline (60s notice, then the existing hard-expiry path). Members never preempt Members; anonymous never preempt anyone. `CapacityExhausted` stays typed and gains `{position, class}`.
3. **Guest compute class.** Anonymous work runs **Opportunistic-only** with a small Compute grant — guests never contend with production (a hole both prior designs left open; closed here).
4. **Claim-on-signup.** A guest lands in chat-zoom on a fresh Zero project. If they sign up within the lease TTL, the guest project binds to their new Member principal — the funnel keeps its best converts. Anonymous data not claimed is wiped at lease end (today's behavior).
5. **Count is a parameter.** `guest_profiles = N` in `appliance.toml` replaces hard-coded `g01..g10`. Owner-locked capacity semantics survive: leases are frontend slots, not GPUs; guaranteed guest compute remains 1 CPU + 1 exclusive fleet-wide GPU block; lease 11 = typed capacity exhaustion, never queue-jump or local fallback; GPU absence fails closed (no slot-local execution, no CPU fallback, no shell launcher).
---

## 3. The frame — one shell, three lenses, chat everywhere

### 3.1 The shell

One SPA on jeryu-web's stack (React 19, zod 4, one pinned Vite, `styles/tokens.css`). The jeryu-web `AppShell.tsx` grid gains a fourth region — the **chat dock** — hosting jain-web's `Feed.tsx` + `Composer.tsx` + `useSessionSocket.ts` (ported to React 19 / zod 4). The dock is the active session, always; it is resizable, collapsible to a two-line ticker, and zoomable to full-bleed. A guest uploading a CSV sees essentially today's jain-web and nothing scarier, because a fresh Zero project defaults to chat-zoom.

```
┌─ Jain ── churn-ds ──────────────────────────────────────  admin@atomicsoul ─┐
│ [1]HOME  [2]CODE  [3]KNOWLEDGE                        ⌘K  ?   ● hub  scq:9/9 │
├────────────┬─────────────────────────────────────────────┬──────────────────┤
│ LEFT PANE  │                 STAGE                        │ CHAT DOCK        │
│ ┌────────┐ │        (the active lens renders here)        │ ┌──────────────┐ │
│ │projects│ │                                              │ │ virtualized  │ │
│ │sessions│ │                                              │ │ session feed │ │
│ ├────────┤ │                                              │ ├──────────────┤ │
│ │ view   │ │                                              │ │ > composer_  │ │
│ │ navig. │ │                                              │ └──────────────┘ │
│ └────────┘ │                                              │                  │
├────────────┴─────────────────────────────────────────────┴──────────────────┤
│ r-1189 training 12/40 ▓▓▓▓▓░ · audits:2red · prs:1 · Esc=nav i=chat 0=zoom  │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Left-pane invariant** (this is how "keep left pane + chat on all views" is satisfied literally and identically): the top half is ALWAYS the project switcher + the active project's session list (jain-web `Sidebar.tsx` behavior verbatim); the bottom half is the view-specific navigator.

**Modal keyboard model** (vim-flavored — the TUI soul; built on jeryu's `useKeyboard.ts`, which already supports chords):

- `1` `2` `3` switch lens; `0` or `` ` `` toggles chat-zoom.
- `i` focus composer (insert); `Esc` back to navigation. The composer is the command line of the whole product — every slash verb works from every view.
- `j`/`k` row nav, `Enter` drill, `[` `]` prev/next project, `{` `}` prev/next session, `⌘K` palette (`CommandPalette.tsx` + `commandStore.ts`), `⌘B` toggle left pane, `?` shortcut overlay.
- `g`-chords for second-order surfaces as drawers on the current lens (never separate destinations): `g f` fleet, `g s` settings, `g a` audit log, `g n` notifications.

**Click parity — every switch has both a keystroke and a pointer path** (the brief asks for "keystroke shortcut or click"). The header lens tabs (`[1]HOME` `[2]CODE` `[3]KNOWLEDGE`) are click targets equivalent to `1`/`2`/`3`; left-pane project rows and session rows are click-to-select equivalent to `[`/`]` and `{`/`}`; attention-queue rows, portfolio cells, and each row's inline verb are clickable equivalents of `Enter`/`a`/`x`; drawer glyphs in the header open the same surfaces as the `g`-chords. Nothing in the product is keyboard-only or pointer-only — the two input models are exact peers, which is what lets a power user fly and a newcomer click.

Landing (Codex §4.1, adopted): admins land on the deployment-wide Runner Board (a HOME variant); members land on their last project+lens; guests land in the project shared by their link.

### 3.2 HOME — portfolio + attention queue

```
├─ LEFT ─────────┬─ STAGE: ATTENTION (7) ────────────────────────────┬─ CHAT ─┤
│ ▾ PROJECTS 14  │ ▌REL  jain-core release blocked · 2 consumers      │  ...   │
│  ● churn-ds    │       stale             Enter=open   a=fix-plan    │        │
│  ● jain        │ ▌AUD  jain-llm jankurai 71<85 hard:1               │        │
│  ○ genomics    │                         Enter=findings a=assign    │        │
│ ▾ SESSIONS     │ ▌R&D  churn-ds ocean done · 4102 feats · auc .911  │        │
│  # explore-1 ● │       (+.021)           Enter=review  b=boil-down  │        │
│  # fix-audit   │ ▌SPL  jain umbrella 120k LOC · split candidate     │        │
│ ▾ FILTERS      │ ▌PR   jeryu-web #212 awaiting approval   a=approve │        │
│  releases  1   │ ▌TOOL jankurai pin drift · 3 repos       a=repin  │        │
│  audits    2   │ ▌NODE xbabe2 quarantined · no kill-ack  Enter=why  │        │
│  r&d       1   │ ── PORTFOLIO ──────────────────────────────────── │        │
│  fleet     1   │ name      lad  sess runs prs aud  rel  knw        │        │
│                │ churn-ds  zero  1    2    -   ok   -    1.8k       │        │
│                │ jain      core  6    9    3   RED  w4   12k        │> _     │
└────────────────┴───────────────────────────────────────────────────┴────────┘
```

- **Left navigator:** attention filters with live counts + a fleet-health mini-strip.
- **Stage:** the virtualized attention queue (severity-sorted) over a dense portfolio table. Every row carries its one-key verb inline: `a` = the contextual act (approve PR / assign an audit-fix agent — which spawns a session pre-prompted from the jankurai findings / repin a tool / open a fix-plan chat), `x` = dismiss/snooze, `Enter` = drill (navigates to CODE or KNOWLEDGE with context loaded; the chat dock switches to the relevant session).
- **Implemented from existing code:** `DashboardPage.tsx` (skeleton), `NotificationInbox.tsx`/`notificationFilters.ts` (queue mechanics), `WorkPage.tsx`/`workModel.ts`, `PullRoomPage.tsx`/`pullRoomModel.ts` (PR items + drift), `JankuraiScoreBadge.tsx`, `RepoHealthPill.tsx`, `RepoTable.tsx`/`familyRollup.ts` (portfolio), `FleetPage.tsx`/`fleetModel.*.ts` (fleet drawer), jain-web `ClusterQueuePanel.tsx` (run rows). The jeryu-tui `mission` lens is the semantic ancestor; its taxonomy ports to `AttentionItem`.
- **Deployment-admin variant** (the admin landing) additionally shows worker health, utilization, storage placement, guest-seat usage, per-project budgets, and durability shortfalls — aggregated, never revealing private content.

### 3.3 CODE — repo, PRs, checks, live agents

```
├─ LEFT ─────────┬─ STAGE: CODE  jain/feat-core · PR #212 ───────────┬─ CHAT ─┤
│ ▾ SESSIONS     │ ┌ FILES │ DIFF │ CHECKS │ TERM ┐   e/d/c/t         │ agent  │
│  # fix-audit ● │ │                                │                │ turn   │
│ ▾ BRANCHES     │ │   Monaco editor / diff (lazy)  │                │ feed   │
│  main 🔒       │ │                                │                │ (live) │
│  cell/fix-a ●  │ └────────────────────────────────┘                │        │
│ ▾ PULLS        │ CHECKS  feat-core/required ✓ · jankurai/proof ✗   │        │
│  #212 open ✗   │         score 71/85 hard:1   Enter=findings       │        │
│ ▾ AGENT RUNS   │ TERM    [xterm tty · agent live · budget 62%]     │        │
│  r-1201 T3 ●   │                                                   │> nudge_│
│                │ o=pr  a=approve  m=merge(protected)  p=pause k=kill        │
└────────────────┴───────────────────────────────────────────────────┴────────┘
```

- **Left navigator:** branches (protection-lock glyphs), open PRs with check state, live agent runs with tier/budget.
- **Stage tabs:** FILES (`RepositoryCodePage.tsx`, `RepositoryFilePage.tsx`, `useRepoTree.ts`, `useBlob.ts`), DIFF (`PullRequestPage.tsx`, `usePrDiff.ts`, `usePrThreads.ts`), CHECKS (`usePrChecks.ts` + evidence receipts; jankurai findings expand inline), TERM (`AgentTerminal.tsx`/`agentControlTransport.ts` → jeryu-agentbridge `pty_driver.rs` verbs).
- **Chat drives the agent:** when the active session has an `agent`-lane Run, the composer routes plain text as `InjectPrompt` and renders the agent's turns in the dock — live coding without leaving the frame. Worktree bans, shell-command policy, and path-scoped grants are runner facts; the UI's job is to render the denial receipts inline in the feed ("blocked: git worktree — policy") so governance is visible, not mysterious (§8).
- **jain session-repo verbs** (`/checkpoint /revert /repo /history`) operate on the same forge repo and render as ordinary commits/refs — one git story.
- **Merge is the protected fast-forward only:** the `m` verb calls the same reviewed lifecycle splitctl uses; there is no second merge path, and the button renders **disabled-with-reason** until `<repo>/required` + `jankurai/proof` + approval are green (protection state from `protection.rs` via API).

### 3.4 KNOWLEDGE — the ocean, the leaderboards, the boil-down

```
├─ LEFT ─────────┬─ STAGE: KNOWLEDGE  churn-ds ──────────────────────┬─ CHAT ─┤
│ ▾ COLLECTIONS  │ ┌ OCEAN │ SOURCES │ CONCEPTS │ GRAPH ┐             │        │
│  ocean-3  4102 │ LEADERBOARD  gen 38/300 · auc · cv 5-fold          │        │
│  distilled  17 │  1 gp_f81 ratio(x3,x9)·log(x12)   .9112 ████████▌  │        │
│  archived   2  │  2 hyperion c112 depth7           .9098 ████████▏  │        │
│ ▾ EPOCHS       │  3 gp_f12 x4^2/x7                 .9081 ███████▊   │        │
│  e12 ▲ .911    │  ── row Enter: formula · provenance · ablation ──  │        │
│  e11   .909    │ SOURCES 1,872 · papers 214 · web 1,190 · code 468  │        │
│  e9    .902    │  [arxiv] Deep feature synthesis…  ↗ cited-by e9    │        │
│ ▾ SAVED Q      │  [github] churn-modeling kit  ↗ dup-risk           │        │
│  "churn surv"  │ TIMELINE e1───e5───e9──▲e12   (metric sparkline)   │        │
│                │ b=BOIL DOWN  s=search  p=pin  x=archive  n=note    │> _     │
└────────────────┴───────────────────────────────────────────────────┴────────┘
```

- **Left navigator:** collections (oceans, distilled sets, archives) with cardinalities; evolution epochs with metric deltas; saved queries.
- **Stage tabs:** OCEAN — leaderboards rendered by ported jain-web chart components (`GpWinners.tsx`, `FeatureImportance.tsx`, `HyperionSweepLive.tsx`, `CvHeatmap.tsx`, `LiftBar.tsx`, `MetricsPanel.tsx`, `InventionLab.tsx`); SOURCES — a virtualized table over the agent-search `ProvenanceStore` (provider badge, dedupe cluster, used-in links); CONCEPTS — cogcore concepts/lessons with FSRS due-state; GRAPH — the one component salvaged from jekko-web (@xyflow/react + elkjs reasoning graph) plus jeryu `IntelligencePage.tsx` codegraph for code-knowledge. Per the critic, GRAPH is **deferred to v1.1**: OCEAN/SOURCES/CONCEPTS render as tables first; the graph is a lazy chunk added once the tables are load-bearing.
- **BOIL DOWN** is the signature verb: select a leaderboard (or let the Evolver's default policy pick), press `b` → a confirmation card in chat ("distill ocean-3 → 24 features, 2 models, archive 4,078") → creates a `train`-lane distill Run → artifacts land in `/features` + `/models` of the project repo **as a PR**. Distillation rides the same reviewed lifecycle as code; the ocean collection flips to `archived` with the artifact digest as its tombstone; the epoch timeline gains a marker. "Boil the ocean, then boil it down to only the best" is thus one verb over the shared machinery.
- **Evolution timeline:** epochs are just distill-run receipts over time; `Enter` on an epoch diffs its feature set against the previous.

### 3.5 The TUI-future aesthetic and performance budget

**Token system: jeryu-web `styles/tokens.css` wins outright.** It already encodes the doctrine — radius 0 everywhere, crisp 1px frame borders with neon-glow composites, mono-first `--font-display`, a near-black multi-neon dark default, a pure-black high-contrast theme, a faint `--color-grid` terminal grid, gold reserved for the tool control plane. jain-web's `design-tokens.css` maps onto it mechanically; jain's large `styles.css` is mined for the feed/composer/chart rules and the rest dies. Ship **dark + high-contrast**; drop the light theme (a maintenance surface with no payoff for "the future, not a website"). "Not a website" means: no rounded cards, no drop shadows, no hero whitespace — every panel is a boxed frame with a mono `label.in.dots` caption, TUI-grade data density, and a status bar always narrating what the machines are doing.

**Performance budget (CI-enforced via `size-limit` + a Lighthouse/memory gate — "FAST as fuck, minimal memory" made into numbers):**

| Budget | Target | Technique |
|---|---|---|
| Core chunk (shell + HOME + chat spine) | ≤ 250 KB gzip | Monaco / xterm / xyflow+elkjs / d3 are lazy chunks loaded per tab; Storybook never ships |
| Fonts | zero downloads | system mono stack from tokens.css |
| First paint (LAN) | < 150 ms | brotli-precompressed SPA served by the fused hub binary; inline critical tokens.css |
| Interactive | < 400 ms | — |
| Lens switch | < 50 ms | lenses are routes but zustand stores (`realtimeStore`, `selectionStore`, new `attentionStore`, ported jain `useCockpitState`) hold data, so remount is render-only; HOME stays warm |
| Heap (10k feed events + 5k knowledge rows) | < 200 MB | virtualization on every list > 50 rows (`react-virtual` already present), windowed event cache (jain `MAX_RENDERED_EVENT_LINES` pattern) |
| WebSocket connections | 1 | merge jain `useSessionSocket` (seq-dedup + REST backfill) with jeryu `realtimeStore` into a single multiplexed topic socket; reconnect never loses order |

**Break-glass TUI:** `jeryu-tui` survives as a **5-lens, feature-frozen ops console** (mission, release, queue, git, jankurai) that reads the local RedlineDB **directly** (not via the API) so it is usable when the web/API is down — a DR requirement that pairs with the shadow master (§7). The other 13 lenses and `jain-tui`/`jekko-tui` are deleted (§13).

---

## 4. The one worker

### 4.1 Spine vs layer — the decision

**SCQ is the spine.** It owns durable job state (`jain-smartcluster-daemon`, redb, deterministic recovery including `FailureClass::DaemonRestart`), fenced leases (`LeaseScope`, epochs), the owner-locked preemption contract (0s hard-kill, push-cancel ≤500 ms p99, rich `LeaseCancelAck` with cgroup-empty / GPU-released / scratch-removed proof, no-ACK ⇒ node quarantine with beneficiary withheld — all typed today in `jain-smartcluster-core/src/fabric/node_protocol.rs`), the outbound-only mTLS transport with durable directional replay (`scq-edge/src/session.rs`), enrolled ed25519 identities, and trust/data-class placement.

**jeryu-ci-runner is the layer.** Its `TrustTier` (T0–T5, `runner-core/src/trust.rs`) and `RunnerClass` (native-rust-hot/-clean, agent-guard, release-hermetic, microvm-rust, oci-docker) plus `select_runner()` become the node-local **execution planner**: given a leased WorkUnit, the node picks the isolation envelope. `jeryu-runnerd`'s own claim/lease/heartbeat loop is deleted — SCQ already does that better (journaled, fenced, replayed). Workcells, warm pool, `jeryu-sandbox-linux`, and `jeryu-agentbridge` are kept intact as the things SCQ never had.

This is the exact spine/layer split both the systems architect and the platform architect converged on independently, and it matches the Codex spec's SM-004 (SmartCluster sole scheduler; other schedulers become projections) — adopted.

### 4.2 The fusion point — `WorkUnitV1`

One type in `jain-fabric` (evolved from `jain-smartcluster-core/src/workload.rs` + `job.rs`, with `jeryu-runner-core` trust/policy types merged in):

```rust
WorkUnitV1 {
  id: JobId, idempotency_key,
  workload: TypedWorkload,           // never argv for non-owner principals
  tier: PriorityClass,               // System | Interactive | Batch | Opportunistic (unchanged discriminants)
  trust: TrustTier,                  // T0..T5 (unchanged)
  policy: { restart_safe: bool, checkpoint_capable: bool,  // ORTHOGONAL (SCQ_RND_ONBOARDING Finding 2.2)
            retry: RetryPolicy, deadline, budgets },
  grants: Vec<Grant>,                // §8.4 dynamic R&D data grants
  placement: PlacementPolicy,        // scheduler-owned; no node= affinity for R&D
  submitter: Principal,              // user | agent-session | system(merge-queue, repair, scrub)
}
```

`TypedWorkload` is the union of registered families, each fail-closed against a versioned allowlist (the existing `jain-worker` ALLOWLIST pattern, extended to every family — the owner-locked "all R&D via typed ResearchJobSpecV1, no argv/host-path crosses the API" rule generalizes to the whole product):

- `Jain{template}` — production product ops (Training, Prediction, DatasetProfile, FeatureExpansion, ReportGeneration, Export, Chat, Research, ManagedBrowser, JopeEncode/Propose, Lime*) — **exists today**.
- `CiJobV1{ir_digest, repo, sha, checks}` — compiled by `jain-ci` from CI IR; immutable input digests.
- `ResearchJobSpecV1` / `JopeWorkloadV1` / `ApexMeasureV1|CompareV1|PromoteV1` — per `SCQ_RND_ONBOARDING.md`, digests only.
- `AgentSessionV1{repo, base_ref, agent_cli, task_ref, interactive}` — live coding (§8).
- `BrowserSandboxV1{bundle_digest}` — jailgun air-gapped browser cells.
- `RepairV1 | ScrubV1{stripe refs}` — storage self-maintenance (§6), submitted by the hub itself.
- `AdministratorArgv` — retained, **owner-principal only**, exactly as today; `/api/v2` and every new adapter resolve a typed template and cannot use it as an escape hatch (Codex §7.2, adopted).

### 4.3 The node — lanes and "all-consuming"

`jainnode` is the sole execution authority on a machine. There is no other path onto it: no SSH work, no docker daemon, no second runner (the owner-locked "raw exec/SSH/direct-Docker/CUDA/mutable-node-config forbidden" rule). Admission — at enrollment and every reconnect — verifies exclusive GPU ownership (a planted foreign CUDA process fails admission; the drill exists), NVML headroom, PSI, disk headroom, clock skew, and a kernel probe (cgroup v2, landlock ABI, seccomp, optional `/dev/kvm` → `NodeCapabilities` flags).

The node owns `jain.slice` at the cgroup root (via the verified delegated-cgroup recipe — `Delegate=memory pids cpu` drop-in + `loginctl enable-linger`, from `jeryu-deploy/ops/security/jeryu-runnerd.service`; without it, `require_cgroup` agent jobs fail closed) and runs four **lanes** — static scopes, not separate daemons:

- `ci` — workcells (physical clones only; worktrees mechanically impossible, §8), warm pool, runner class per `select_runner(trust)`.
- `prod` — production CPU/GPU work (training epochs, inference, canaries) via the typed jain-worker executor; also `TrustedService` allocations (e.g. starforge serving a pinned model bundle).
- `rnd` — R&D typed workloads, always preemptible.
- `agent` — live-coding cells (agentbridge) and air-gapped browser sandboxes.

Lanes have configured ceilings but **any lane may claim the whole machine** ("all-consuming"): capacity is one vector (cpu, mem, gpus, scratch); lanes are just labels on leases (this is why `lane` on `Run` is a UI label, §1.3). Cross-lane arbitration is decided at the hub and **enforced at the node by resource class** — one rule for the whole "dynamically kills/pauses queued work across lanes" requirement:

- **GPU contention → kill.** GPU state is not freezable. Production (System/Interactive) preempting Opportunistic issues `LeaseCancel{checkpoint_deadline_ms:0, terminate_by_ms:0, kill_by_ms:small}` — the locked 0s hard-kill. The victim requeues `Running→RetryWait→Queued` with `FailureClass::Preempted` (never charges the failure budget — `consumes_failure_budget()` already returns false) and resumes from its last durable checkpoint receipt (digest-bound CAS artifact).
- **CPU/mem contention → freeze Batch, kill Opportunistic.** Batch work (bulk CI, unattended agent coding, checkpointed R&D) is paused with the cgroup **freezer** (memory retained, resumed when pressure clears) — this is the brief's "pause"; Opportunistic (ocean sweeps, scrub, guest compute) is killed as above.

Cancel is push-path: the hub keeps a durable per-node push queue (controller side of the replay journal); delivery p99 ≤ 500 ms, replayed after reconnect. Cancel completion requires the rich `LeaseCancelAck` (`cleanup_complete()` = cgroup empty ∧ GPU released ∧ scratch removed). **No ACK ⇒ node quarantined, capacity withheld** — the beneficiary is never placed onto a node with an unproven kill; foreign ownership drains/quarantines and pages the operator rather than broad-killing (owner-locked). All of this exists in `node_protocol.rs` and is adopted verbatim as product-wide law, now covering CI and agent lanes too.

### 4.4 Tiers — how preemption is settled

The critic's tier assignment (resolving the one place the two systems designs disagreed) is binding:

| Tier | Work | Pressure response |
|---|---|---|
| **System** | hub-internal only: repair, scrub, fencing, owner emergencies | never preempted |
| **Production** | production serving/training (reserved, non-preemptible by ordinary R&D — Codex SM + owner lock) | reclaim is not a failure |
| **Interactive** | attended agent sessions, merge-gating CI (a human/release is waiting) | may preempt Batch/Opportunistic |
| **Batch** | bulk CI, unattended agent coding, checkpointed R&D | **freezable** under CPU/mem pressure |
| **Opportunistic** | ocean sweeps, scrub overflow, guest compute | **0s hard-kill**, requeue from checkpoint |

`Production` is appended as a new wire variant between System and Interactive; existing System/Interactive/Batch/Opportunistic discriminants **do not move** (Codex §7.2 compatibility rule, adopted). This settles the earlier disagreement (one design put CI at System, another at Batch): CI splits by whether something is waiting on it.

### 4.5 The hub scheduler and the one ledger

`jain-sched` (scqd's deterministic core, extracted from `jain-smartcluster-daemon`) is the only scheduler. Four submitters, all producing WorkUnits: (1) the web/chat surface (user `/start`, uploads, ad-hoc R&D); (2) `jain-ci`'s merge queue + push events (compiles CI IR → `CiJobV1`; the merge queue **proposes**, the scheduler **places** — jeryu-ci-scheduler's own lease code is deleted); (3) the Evolver and autonomous innovation loops (autonomous R&D cycles are `ResearchJobSpecV1` submissions from the product's own agents through the same governed API — the R5 submit-chokepoint seam is preserved); (4) the hub itself (Repair/Scrub/System services).

Placement = existing `PlacementPolicy` + trust/data-class constraints + a new **locality hint** from the shard map (prefer nodes already holding input stripes, §6). Inputs flow over the existing `InputManifest`/`InputChunkRequest`/`InputChunk` streaming protocol, now backed by the shard layer.

**Central tracking — the one ledger.** One `work` ledger in RedlineDB: every WorkUnit with submitter, session linkage, lease history, receipts (LeaseCancelAcks, checkpoint digests, preemption KPIs per `preemption-drill.kpi-schema.json`, jankurai proofs, agent transcripts as artifacts), outputs, and cost (gpu-seconds / cpu-seconds). The scheduler's redb journal stays its private recovery truth; the RedlineDB ledger is the mirrored product-visible truth the portfolio view reads. **Nothing executes anywhere without a row here** — that is the mechanical meaning of "ALL code execution, including autonomous innovation R&D and ad-hoc R&D, goes through the central hub for tracking."
---

## 5. Install and enrollment

### 5.1 The dead-simple flow

The whole install story is: download one artifact, run the hub, tell the hub which machines it may use.

```
# Hub (first machine):
$ curl -LO https://downloads.neverhuman.org/jain/jainhub && chmod +x jainhub
$ ./jainhub init --data /var/lib/jain      # creates RedlineDB, master key, forge, admin token
$ ./jainhub serve                          # 127.0.0.1:8787 API/SPA/git ; :9443 fabric (mTLS)
```

Add a worker — two flavors of one enrollment:

```
# assisted (hub SSHes ONCE to bootstrap, then never again as a work path):
$ jain node add ubuntu@xbabe1 --lanes ci,prod,rnd,agent --scratch 400G --shard 2T
   → copies jainnode + a one-time token; installs the systemd unit; the node enrolls
     outbound; the SSH channel is closed and is never used for work.

# manual (air-gapped / no SSH):
$ jain node invite xbabe1                   # prints a one-time token (10-min TTL, single use) + fabric URL
node$ ./jainnode enroll --hub https://hub.lan:9443 --token <TOKEN>
node$ ./jainnode install-service
```

`enroll` generates the node's ed25519 identity, presents the token over the outbound TLS channel, the hub runs **admission** (§4.3), then issues an mTLS client cert bound to the identity and records a `NodeRecord` + capabilities. From then on the node maintains a single outbound mTLS session (scq-edge semantics kept verbatim: newest connection wins, both directions journaled and replayed, direction/identity checked). Ticket bytes are read from protected stdin or an already-open fd — never argv, env, or a world-readable file (Codex §5.3, adopted).

**Firewall story:** nodes need exactly one outbound port to the hub; the hub needs zero inbound to nodes. This is what makes enrollment NAT/firewall-trivial and why folding scq-edge into the node (making every node an "edge" node) lets the `scq-edge` binary die.

Operator surface (the `jain` CLI is the hub binary in client mode — argv0 alias): `jain nodes ls|show`, `jain node drain|cordon|rm`, `jain work ls|show|logs|cancel|hold`, `jain tui` (the break-glass TUI rides the same API). Over time this CLI absorbs `scq`'s verbs and then splitctl's release verbs.

### 5.2 Fleet reality and the appliance

The appliance/single-box compose ships `jainhub` + `jainnode` on the same host (the node enrolls via localhost with a pre-baked token). **jain-web's inline-CPU fallback and Unix-socket scqd dispatch retire — there is always a node, even on one box.** This removes the appliance-vs-cloud dispatch divergence.

The current fleet is grounded truth, not aspiration: production is **AtomicSoul** (orchestration-only, **no local GPU** — owner gate 2026-07-14; GPU proof must be typed SmartCluster-dispatched) + **xbabe1** + **xbabe3** (GPU workers). **xbabe2** is drained (97% disk; the box this spec was written on) and rejoins automatically on green admission only — no manual pool edit, ever. AtomicSoul production is the same compose as any customer, with more nodes and `guest_profiles = N` set (§11).

Controller-eligibility is an admin-approved property of a `jainhub` identity, distinct from worker enrollment: a worker advertisement can never grant controller/consensus membership (Codex §5.4, adopted; relevant to the shadow master, §7).

---

## 6. Distributed storage — `jain-shard`

### 6.1 One CAS, extended — not a second one

SCQ v2 already has a content-addressed encrypted artifact store (`jain-smartcluster-daemon/src/artifact_store.rs`: whole-file sha256 addressing, offset-sequential XChaCha20-Poly1305 chunk files, 7-day retention, refcounts). Verified fact: **no erasure-coding, chunk-dedup, or compression exists anywhere in the three trees** (reed-solomon / raptorq / erasure / blake3 = zero hits). The storage fabric is therefore an honest greenfield built as a **striping layer beneath the existing CAS**, not a rewrite and not a rival store. `jain-shard` is the one genuinely new storage crate; `reed-solomon-simd` and `zstd` are the only new heavy deps.

### 6.2 The write path

Client/job hands the hub an object → hub computes the artifact id → generates a per-artifact key (wrapped by the hub master key from `init`; nodes never see plaintext or keys) → **compresses (zstd level 3)** → encrypts the stream (XChaCha20-Poly1305) → splits ciphertext into fixed 8 MiB chunks → groups *k* chunks per stripe → Reed-Solomon produces *m* parity shards → places *k + m* shards on *k + m* distinct nodes via **weighted rendezvous hashing** (HRW over enrolled node ids, weighted by free shard budget) → records a `StripeManifest{artifact_id, chunk_size, stripes[{k, m, shards[{shard_id, node_id, checksum}]}]}` in RedlineDB. Nodes verify the shard checksum on receipt and on read; nodes store **ciphertext-only** shards, so a stolen or decommissioned disk leaks nothing (the accepted price is no cross-chunk dedup; whole-object dedup via the plaintext artifact id stands).

Placement is **hub-owned** and recorded in RedlineDB — no gossip, no DHT, no second consensus. The hub is already the single metadata authority for scheduling, so it is also the one for placement, and the scheduler gets input-locality hints for free (§4.5).

### 6.3 The risk knob — a durability profile, never raw k+m

The user sets tolerance per dataset/project (`jain data put --profile durable`, or in the UI); the hub derives (k, m) from the live cluster:

| Profile | Survives | Derivation |
|---|---|---|
| `scratch` | 0 nodes | local-only, no stripes, TTL-bound |
| `standard` (default) | 1 node | m=1; k = min(6, live_nodes − 1) |
| `durable` | 2 nodes | m=2; k = min(6, live_nodes − 2) |
| `critical` | 3 nodes | m=3; k = min(4, live_nodes − 3); requires a configured offline/export target |

If `live_nodes < k+m`, the hub **degrades honestly to (m+1)-way replication** and reports it — no fake erasure coding on 2–3 node clusters. On a 10-node cluster, `durable` costs ~1.33× instead of 3× replication; that is the entire point of "redundancy without 1-to-1 replication, cluster-wide compression." Data classes can force floors (release artifacts ≥ durable). Every project storage surface continuously displays requested vs. achieved node/rack/site durability, degradation reason, and repair backlog; the system never silently claims a level the topology cannot satisfy (Codex §10.3, adopted). Failure-domain (rack/site) labels require admin attestation — a worker's self-report cannot establish durability.

### 6.4 Reads, repair, scrub

- **Reads (v1):** hub-gateway — the hub reconstructs from any *k* shards and streams via the existing `InputChunk` protocol. Erasure coding buys durability, not bandwidth. **v1.1** adds peer reads via short-lived signed chunk-read capabilities over the same mTLS identities; the scheduler's locality hint already makes most reads local. (Per the critic, peer reads are cut from v1 until hub-gateway reads are a measured bottleneck.)
- **Repair & scrub are just WorkUnits in the one scheduler** (§4). Node heartbeats carry a cheap shard-inventory epoch + count and answer exact range queries. When a node's lease expires past a grace window, the hub emits `RepairV1` WorkUnits (System tier, bandwidth-throttled) that reconstruct lost shards from any surviving *k* and place them on new nodes. `ScrubV1` runs Opportunistic: read shard, verify checksum, report; corruption triggers a point repair. Drills (§12): kill a node holding shards → all `standard+` objects readable and re-repaired to full redundancy; flip bytes in a shard → scrub detects and repairs with a receipt.

### 6.5 Who rides the layer, and who never does

- **Datasets** — first-class: uploaded → striped; jobs receive them via `InputManifest` with locality-aware placement.
- **Model bundles** (`/opt/jain/model-bundle`, ~1.1 GB today) — published `critical`; nodes serving inference **pin** the full plaintext under a lease-scoped grant (a pin cache, not a copy outside the system).
- **Knowledge artifacts** (thousands, small) — v1 stores them as RedlineDB rows + ordinary striped artifacts; ≤64 MiB pack-file packing is a **deferred** optimization (critic cut), not v1.
- **Build cache** — `jeryu-cache` keeps its receipt/poisoning-defense front end; its storage backend becomes shard-layer `standard`.
- **R&D checkpoints** — already digest-bound; the digests now point at `standard` striped artifacts, which is what makes 0s hard-kill + requeue durable across node loss.
- **NEVER on the layer:** RedlineDB and live git bare stores (circular dependency — the shard map lives *in* RedlineDB; they replicate via §7 and only their periodic snapshots/bundles archive into the shard layer), workcell checkouts, warm pools, scratch, PTY buffers.

Deliberate v1 non-goals: no chunk-level dedup (ciphertext defeats it), no geo-placement, no rebalance-on-add beyond HRW's natural drift (a manual `jain shard rebalance` exists but is deferred).

---

## 7. Shadow master — control-plane failover

### 7.1 The decision: epoch fencing v1, Raft deferred (owner OD-3)

v1 is a **designated shadow, owner-promoted, fleet-fenced** — no Raft, no etcd, no election, zero new consensus code. This is the owner's explicit choice over the Codex spec's OpenRaft design (SM-011/012), for one decisive reason: **the current healthy fleet is two nodes**, and automatic Raft promotion needs ≥3 controller-eligible nodes — inert on today's topology. Epoch fencing reuses the restart-monotonic-epoch pattern that `guestd` and SCQ already ship, works on two nodes, and gives RPO 0 on the release history that owner law protects. Raft/OpenRaft is recorded as the explicit **v2 upgrade path** once ≥3 controller-eligible nodes are standard (§15, SM-011/012 disposition).

### 7.2 What must be replicated (two truths)

A shadow master must replicate **both** authoritative stores (verified: they are separate truths):

1. **RedlineDB** — the forge relational state (users/orgs/repos/PRs/checks/protection/grants/receipts/scheduler projection). Files like `forge.sqlite`/`jeryu.redline` (~55 MB today).
2. **The bare git object/ref stores** — `~/.jeryu/repo-mirrors` (~408 MB today), authoritative via `jeryu-gitd`'s ref service, separate from RedlineDB.

Setup: `jain hub shadow set <node|host>` — the shadow runs `jainhub follow --of https://hub:9443` (same binary, follower mode; may co-reside with a `jainnode`). Two replication streams, both **outbound-from-shadow** (same dial-out discipline as workers):

1. **Git:** every ref update in `jeryu-gitd` fires the mirror hook (`jeryu-mirror`'s backup/replay engine, repurposed — it exists as snapshot DR transport and is promoted to a streaming hook): **synchronous for protected refs and immutable tags** (the push ack waits for the shadow's fsync — release history RPO 0, owner law) and asynchronous (seconds) for all other refs. Objects stream as packfiles.
2. **RedlineDB:** v1 interim = snapshot-every-N-minutes + a bounded op-log of session events, with the RPO **measured and surfaced** on the hub status bar. `redline-repl` (a small WAL-segment shipping feature) lands in `redline-core` **after** the single-homing (§10) so it lands once under one lock, not the dual-lock ceremony.

### 7.3 Fencing and promotion

Fencing is a monotonic **hub-epoch** (u64). Every node stores the highest hub-epoch it has ever acknowledged in its durable journal and refuses any hub session presenting a lower epoch. Promotion (`jain hub promote`, run on the shadow by the paged operator/agent — deliberately manual in v1):

1. The shadow increments the epoch to E+1 and asks all enrolled nodes to fence E+1.
2. Promotion **commits only when a majority of enrolled nodes acknowledge** — the worker fleet is the witness set (2-node cluster: quorum = 1 node; single-box appliance: shadow mode is meaningless and documented as such).
3. Once fenced, the old master — even if alive across a partition — is rejected by every quorum node **and** by gitd's protected compare-and-swap ref service (writes require the highest epoch). It cannot schedule, cannot move protected refs, cannot mint guest leases (guest epochs are subordinate to the hub-epoch). This closes the split-brain hole cleanly with the fencing pattern already in the tree.
4. Nodes reconnect to the new hub and replay their journals; the scheduler reconciles exactly as it does for `FailureClass::DaemonRestart` today — running leases are adopted or cancelled per policy; preempted work requeues from checkpoints.

### 7.4 Honest v1 limits (stated in the product)

Promotion is a human/agent command, not automatic (the pager fires; auto-promote is v2 once drill history earns trust). RedlineDB RPO is the replication lag (target < 5 s with `redline-repl`, minutes in the interim). Non-protected git refs may lose the last seconds. In-flight chat streams drop and clients re-backfill via the existing seq-dedup + REST reconnect. The old master never fails back automatically — it re-enrolls as the new shadow after `jain hub resync` (wipe + full re-sync). RTO target ≤ 15 min, drilled on a recurring cadence (§12).

---

## 8. Live coding and native governance

This is the section that makes "the product develops itself, inside its own governed runner, and agents can never violate precommit" real — with mechanisms, not policy hopes.

### 8.1 Every development act is a Run

Human-driven or autonomous, every development act on the product itself is an `AgentSessionV1` WorkUnit: scheduled by the hub, leased to a node's `agent` lane, tracked in the one ledger, budgeted (wall-clock + output + token budgets from agentbridge and the jain-agent runbook contracts), Interactive tier (it competes like production and is not killed by a training job, but an owner System job can preempt it — its checkpoint is its git state, always recoverable).

### 8.2 The cell — worktrees impossible by mechanism

`jain-cells` (lifted from `jeryu-runnerd/src/workcell.rs`, `warm_pool.rs`) claims a warm workcell — a **standalone physical clone** (`git clone --no-local --no-checkout` then `checkout --detach <SHA>`; the family-wide worktree ban). Worktrees are banned four independent ways, cheapest first:

1. `.git/worktrees` is pre-created as an empty **root-owned read-only** directory in every cell, so `git worktree add` fails at `mkdir`.
2. `jankurai-guard` / `fscheck` scans the cell for worktree metadata.
3. host-mediated publish rejects any branch state carrying worktree metadata.
4. gitd never serves worktree state.

`jain-bridge` (jeryu-agentbridge, unchanged) launches the agent CLI (Claude/Codex/Jekko via `cli_registry` LaunchPlans) inside `jain-sandbox` (`jeryu-sandbox-linux`, the single fail-closed unsafe island): user+mount+pid+net namespaces, seccomp allowlist, landlock scoped to `{/cell, /grants, /out}`, cgroup budget, watchdog. TrustTier T3 (AgentGuard class), `NetworkPolicy::Deny`.

### 8.3 One egress — the hub

The cell's only network is a loopback socket proxied (`jeryu-egress`, allowlist = exactly two upstreams) to (a) the hub's **jnoccio-fusion gateway** — the agent sees one OpenAI-compatible model endpoint; keys stay server-side in the forge secret store (zyal-key-pool custody moves server-side); provider health/limits are learned from traffic; per-session token budgets enforced at the gateway — and (b) the hub's git smart-HTTP, scoped read-only to the session repo. No other DNS, no other routes. "Aggressive online agent search" runs as separate `Research` WorkUnits through agent-search's provider layer **hub-side**; results become knowledge artifacts the coding cell reads as grants, never the open internet directly. This collapses egress allowlisting, key custody, and provider-health learning into one existing component. `jain-router`'s `routerd` daemon retires; its frozen-ExtraTrees routing signal folds into the gateway.

### 8.4 Dynamic per-path R&D data grants

The WorkUnit's `grants` list materializes as **bind-mounts** under `/grants/<grant-id>` (ro or rw) inside the cell's mount namespace. Because `/grants` is pre-allowed in the landlock ruleset, granting mid-session is a `mount` and revoking is a lazy `umount` — **no landlock mutation** (landlock rulesets are add-only per domain and cannot narrow without re-exec'ing the jail; this is a verified Linux fact and the reason the mount approach wins over the "regenerate landlock rules" alternative), no agent restart, instant and race-free. Flow: agent requests via a tool call → hub surfaces to the owner in the UI (or auto-policy for a pre-approved data class) → hub pushes `GrantUpdate` down the fabric session → node mounts. Every grant is a ledger row with a TTL and receipts. This is the "dynamic read/write permissions for ad-hoc R&D data" ask, done safely.

### 8.5 Bad-shell prohibition — layered, none of it agent-goodwill

- seccomp/landlock make destructive syscalls and out-of-scope paths impossible;
- the `jain-agent` fail-closed tool-authorization contracts become agentbridge's command policy (deny-by-default: no curl, no docker, no `git config` rewrites, no worktree; a receipt for every authorized tool call);
- the in-cell `git` is a thin guard shim with `core.hooksPath` pinned to a root-owned read-only directory holding the precommit hooks, and the argv filter bans re-pointing it — so an agent cannot even *commit* a violation.

### 8.6 Native jankurai enforcement — at the ref service, not the check-run

Verified reality that forces this design: `jankurai-guard` is real and shipped (FUSE/watcher save-gates, an `audit-file` delta gate, a blocking precommit gate that trips on `hard_findings > 0` / marker caps / score regression), **but** its blocking mode is opt-in and `JANKURAI_SKIP_HOOKS=1` bypasses it client-side everywhere, and its OS-hardening backends (landlock/fanotify) are documented no-ops this release. Client-side hooks therefore cannot be the load-bearing gate. The load-bearing gate is **server-side**, where `jeryu-gitd` already has a real pre-receive guard.

Three tiers:

1. **Forge-side (the gate that cannot be bypassed).** A CI job (RunnerClass release-hermetic, TrustTier T0) runs jankurai against the exact PR-head SHA → `repo-score.json`. `jankurai-proofbind` (exists) binds `{commit_sha, repo_id, report_fingerprint, input_fingerprint, policy_fingerprint, auditor_version}` into a **ProofReceipt** signed with the runner's enrolled ed25519 identity — the **same enrollment** as nodes; no second PKI (Codex SM would need a PKI; this reuses SCQ's). The runner POSTs it; `jeryu-proof` gains `verify_receipt(receipt, trust_roots, pinned_policy) -> Verdict` (signature chains to the forge trust root; `policy_fingerprint` matches the repo's pinned `audit-policy.toml` digest; `commit_sha` matches; auditor version satisfies the tool-manifest pin). The gate lands in `jeryu-gitd/src/refs.rs::RefService::merge_pull` — the only code path that advances `refs/heads/main` — which refuses the fast-forward unless `verify_receipt` passes for the exact merge-candidate SHA. **The `jankurai/proof` check-run stays but becomes a rendered view of the receipt, not the authority** — forging the check-run status accomplishes nothing because gitd verifies the receipt itself. Additionally, `PreReceiveGuard::evaluate_lines` (`hooks.rs`) gains a per-repo mode that requires a receipt on `new_oid` for `refs/tags/*` and release-branch patterns (via the existing `ProtectedRefRule` list). Scratch branches stay unencumbered — WIP must not need proofs.
2. **Runner-side (agents cannot violate precommit).** The `ExecPolicy` and grant machinery of §8.2–8.5, enforced in the sandbox boundary. `jankurai-guard` runs in Enforce mode inside the cell as **defense-in-depth**, not as the gate. The backstop is architectural: agents publish via **host-mediated publish** (they hold no push rights), and that endpoint runs `jankurai-guard` on the candidate tree, refusing publish when hard findings > 0 and the repo policy says enforce.
3. **Product-side (surfacing).** The HOME portfolio reads a `repo_health` read-model joining ProofReceipt freshness, `repo-score.json`, open-PR merge blockers (missing receipt = an explicit blocker reason string), and release-lane state. Red = merge-blocking; every red item deep-links to the exact PR/receipt/finding.

**Corrections invariant preserved:** receipts are append-only and SHA-bound; a new commit needs a new receipt; immutable tags never move, so `-split.N+1` corrections carry fresh receipts. `autonomy_bridge` stays the record-only evidence judge; auto-merge stays unshipped (its own 7-probe adversarial review deemed the merge path unsafe — respected).

### 8.7 Publish

The agent never holds push credentials. On `/publish` (or session end), the node ships the branch state as a `LeaseFinish` artifact; the hub applies it to a namespaced branch (`agents/<session>/…`) via its own gitd — host-mediated publish, exactly as jeryu sessions do today — and opens the PR. Gates in order: in-cell `jankurai-guard` (advisory) → `<repo>/required` + `jankurai/proof` forge checks (blocking; `minimum_score=85`, `hard_findings=0`) → protection (1 approval, linear history, enforce_admins) → reviewed FF merge → immutable tag. This is the reviewed no-bypass lifecycle, unchanged.

---

## 9. The Evolver — the standing autonomous loop

This section fills the hole the critic identified: the brief's heart — "agent clusters take over… boil the ocean… keep evolving or archive… aggressive online agent search per project" — needs a **standing decision-maker**, or "agent clusters take over" is false advertising. No design named it; here it is.

### 9.1 What it is

The **Evolver** is a hub-side, per-project **policy loop** — a library, not a daemon — that owns each project's autonomous lifecycle. Every cycle it may: schedule an ocean epoch (feature/model search), trigger an agent-search fan-out, evaluate leaderboard deltas, propose a BOIL DOWN, or archive a dormant project — **all by submitting typed WorkUnits under the project's Compute grant through the same governed API as every human**. It never has a private execution path; it is a submitter (§4.5), so it is preemptible, budgeted, and fully tracked in the one ledger. Every Evolver decision is a chat event in the project session (visible, interruptible) and becomes an AttentionItem when it needs a human (promotion approval, budget exhaustion, contradictory findings).

### 9.2 Built from verified, currently-unused substrate

- **Durable loop state** = `jekko-store`'s daemon subtree (verified rich): `daemon_run → daemon_task → daemon_task_pass → daemon_iteration`, with `daemon_task` carrying a `lane`, `difficulty/risk/readiness` scores, `attempt`/`no_progress` counters, `incubator_round`, and separate `incubator_status`/`blocked_reason`/`status` fields (the normal/incubator/blocked/archive states live across these fields, not in `lane` alone). This schema exists and is load-bearing; what it does **not** yet carry is a fixed pass taxonomy — `daemon_task_pass.pass_type` is a free-form `String`, not an enum. So the Evolver adds **one convention over the existing `pass_type` column**: an 8-stage incubator sequence (scout → idea → strengthen → critic → synthesize → prototype → promotion_review → compress) that is exactly the boil-the-ocean-then-boil-it-down loop. The durable substrate is reused; the pass sequence is the new (small) design layered on top, not a pretence that it already ships.
- **Governance envelope** = `jain-agent` runbook contracts (`GeneratedZyalRunbook`) compiled by `jain-zyal`/zyalc; promotion is **host-evaluated and evidence-gated** with a `model_confidence_cap` that prevents an agent from self-inflating readiness — the host owns the gate, not the agent (Codex §9.3, adopted).
- **Memory** = `cogcore`: episodic/semantic/procedural/**negative** memory capsules with promotion states (scratch/run/project/shared_library) and FSRS decay that fades weak memories; consolidation into `SynthesizedLesson`s is the "boil down to only the best" mechanism (deterministic `RuleBackend` shipped; the LLM backend is deferred — v1 uses the deterministic path). The `forever` tables already bind jankurai findings (`daemon_finding.rule_id`) and regression cycles, so audit health is part of evolution memory.
- **Search** = `agent-search` (16 providers, `ProvenanceStore` with content-hash dedupe + TTL, query router, safety layer that blocks internal URLs and quarantines hostile content). Retrieved content is **data**: it cannot grant tools, change a runbook, broaden authorization, or become executable instructions (Codex §9.4, adopted).
- **InnovationZero engine** = `jain-research`: `ResearchRunRequest{dataset_schema, target_metric, domain_prompt}`, a budget-capped `propose → experiment → write_paper` loop (max candidates/experiments/events, hard caps), promoting to `jain-core/invention-seeds`. This is the dataset-only tier's autonomous cycle (the Move42 InnovationZero method) — the same loop machinery, seeded by a dataset instead of a repo.

### 9.3 One provenance substrate (a named workstream)

Three parallel provenance/evidence stores exist today (jekko daemon SQLite ledger, agent-search provenance SQLite, jain-research in-memory receipts) and are **not** unified. The Evolver's ledger unifies them onto the one `work` ledger + KnowledgeItem CAS (§1.4, §4.5): every ocean epoch, search fan-out, and distillation is a Run with receipts; every source, lesson, and leaderboard entry is a KnowledgeItem with provenance. This is what lets the KNOWLEDGE lens (§3.4), the portfolio (§3.2), and the admin rollup (§2.5) all read one truth.

### 9.4 Evolution and archival

A project can keep kicking along (the Evolver keeps scheduling epochs under budget) or be archived (dormant → the Evolver proposes archive; owner confirms; the ocean's best distillate is retained as a `critical` artifact, the rest expires under policy). Archival is reversible: reopen restores the project's last distilled state and resumes the loop. "Evolution as an ongoing concept, some projects keep going and others archive" is thus a scheduler-budget property, not a separate lifecycle system.
---

## 10. Consolidation — the small-repo doctrine (owner OD-2)

### 10.1 The doctrine

The owner's lens, verbatim: *"With AI we want small code, and small repos, unless the repos are so small they are causing issues (e.g. closing us down with releases)."* This binds **neither** the Codex spec's "repos never consolidate" stance **nor** the design panel's blanket 48→12 monorepo-ward push. The rule is:

> Repos stay small by default (AI-context-sized, small blast radius, independently releasable). A repo merges **only** when it trips an explicit consolidation trigger.

**Consolidation triggers** (a repo must hit one to merge):

- **(a) Lockstep coupling** — it always releases in the same wave with the same pin bump as a sibling; the boundary buys nothing and costs a cross-repo pin dance.
- **(b) Fake repo** — an identity-only scaffold, a vendored copy, or a byte-identical duplicate. These are not repos, they are noise.
- **(c) Release-ceremony stall** — the repo's participation in the release train is demonstrably expensive out of proportion to its content (the motivating example is real: the two-consumer redline dual-lock proof plus the 26-repo × identity-binding waves left 8.0.1 with every repo `identity_status = pending` and no bound release — ceremony, not code, is the cost).
- **(d) Heavy circular cross-pinning** — repos that cannot build without each other's tags in a cycle.

**The primary fix for release friction is automation, not consolidation.** The single biggest lesson from the current tree is that the "one command" release spine is aspirational: `deploy.sh` calls a `release-candidate.sh` / `splitctl release-candidate` that **do not exist**, so wave-driving is manual. Implementing the real orchestrator (wave sequencing, identity binding, evidence aggregation) so that N small repos release **cheaply** is a first-class workstream (§14, M-Ops). Consolidation is the fallback where coupling is genuinely structural.

### 10.2 What the triggers actually catch (the known merges)

| Action | Repos | Trigger |
|---|---|---|
| **Delete** | the 8 jekko identity-only scaffolds (`jekko-{agent,core,deploy,jnoccio,mcp,memory,search,zyal}`); `jain-jekko/` (full vendored copy of the jekko product); thin `jain-{jailgun,jnoccio}` contract duplicates | (b) fake repos — real code lives in the jekko umbrella / the real implementations; delete after a delta-diff proves nothing unabsorbed is lost |
| **Single-home** | `redline` (jain-redline + jeryu-redline; `redline-core` is byte-identical) | (a)+(c) — the dual-lock two-consumer proof is pure ceremony once the trees are proven identical; one repo, one immutable tag both consumers pin |
| **Merge → one worker repo** | `jain-smartcluster` + `jeryu-ci-runner` | (a)+(d) — they co-evolve by design (§4); the spine/layer fusion is one codebase |
| **Merge → one contracts repo** | `jain-contracts`, `jain-domain`, `jeryu-core/crates/domain`, runner-protocol, jain-worker protocol | (a) — the typed cross-repo boundary releases lockstep with everything |
| **Merge → one ops repo** | `jain-split-ops`, `jeryu-release-ops`, `jain-ops` | (a) — three families' release machinery unify into one authority manifest + one release machine |

**What STAYS small and independent** (no trigger): the model backends (`jain-catboost/-lightgbm/-xgboost/-jable`), `jain-core/feat-core`, `jain-math`, `jain-starforge`, `jain-battle-gpu`, `jain-report`, `jain-research`, `jain-model-zoo`, the forge engine crates, the agent-runtime crates, the knowledge crates. These are small, independently releasable, AI-context-sized — exactly what the doctrine wants. Expected end state ≈ high-teens-to-low-20s small repos + the 2 binaries, **not** 12, and **not** 48-forever.

The design panel's 12-repo map is retained only in **Appendix C** as the rejected-alternative it became.

### 10.3 Mechanics (unchanged where merges happen)

- **No history rewrites, no worktrees.** Each merge: physical clone of the absorbed repo, `git merge --allow-unrelated-histories` (or subtree-prefix) into a feature branch of the survivor, PR through normal protection (1 approval + `<repo>/required` + `jankurai/proof` + linear-history FF). History and blame travel.
- **Tombstoning.** The absorbed repo gets a final immutable tag (`<name>-final-split.N`), a tombstone README pointing at the new home, and `state = "absorbed", absorbed_into = "…"` in `repos.manifest.toml`. splitctl gains manifest-state validation: PRs to absorbed repos are refused at `pr-open`.
- **8.0.x undisturbed.** Consolidation waves begin **only after** in-flight 8.0.x lanes reach their immutable tags; the doomed repos stay `state=active` until then.
- **Toolchain reconciliation.** Edition **2024** everywhere (`cargo fix --edition` + review during the absorbing PR); **React 19 + zod 4 + one pinned Vite**. The one risky seam is jain-web's WS `EventSchema` (zod 3, seq-dedup + REST backfill): migrate zod3→4 **in place in jain-web first**, gated by the existing WS contract tests + a recorded-frame replay corpus, so the later absorption is a file move, not a semantics change.
- **Lock hygiene.** Never hand-edit `redline.lock.toml`; the single-homing lands as a normal pin bump through `proof-refresh`, after which the dual-lock flow retires.

### 10.4 Data migration (the piece no design had)

Consolidation moves *repos*; a merger must also move *data*:

- **AtomicSoul's existing owner sessions/artifacts** in jain-web's RedlineDB → the merged Project/Session schema, content-hash verified, chat ordering and actor attribution preserved or explicitly marked unresolved (Codex §14.1, adopted).
- **Existing per-session gix repos** → promoted to real forge repos (§1.1).
- **Existing token owners** → unclaimed migration principals; bootstrap admin claims the local owner; additional principals require an explicit invitation+claim (no heuristic email/token transfer — Codex §14.1, adopted).
- **Existing Jekko reasoning/memory** → imported into a selected project; legacy Global memory becomes `shared_library_pending` (admin review), never directly shared; removed worktree modes import as historical facts only, unresumable (Codex §14.2, adopted).
- **No dual-write period** (Codex SM-013, adopted): import → content-hash + authz verify → shadow-read comparison → bounded maintenance window → final delta + journal cut → activation → retained rollback snapshots.

---

## 11. Deployment and parity

### 11.1 One artifact, two roles, three containers

Base = `jain-deploy/deployment/appliance/docker-compose.appliance.yml` and its owner-locked invariants (digest-pinned images — the `@sha256:0000…` PIN-ME placeholders replaced from signed image receipts, read-only rootfs, non-root, `cap_drop: [ALL]`, no-new-privileges, **no docker.sock**, **no Docker-in-DinD** — the earlier DinD/3-slot plan is superseded by ordinary digest-pinned Compose, Caddy-only edge, internal-only network). Target container set:

- **caddy** — the only edge.
- **jainhub** — forge (SPA + `/api/v1` + `/api/v3` + git + receipts) + scheduler + guest-lease library + model gateway + shard placement + shadow replication, from a scratch image.
- **jainnode** — the worker beside the hub (always present, even single-box).

Embedded RedlineDB is the default (`redlinedb-server` retires as a service; `db-shim` survives for tests). Workers are **never compose services** — node install = download the binary, enroll an ed25519 identity, appear in the node registry (which replaces static `worker-endpoints.toml`; the file survives only as a bootstrap seed). This matches the owner-locked "no standing worker service / GPU absence fails closed" posture and the three-images-one-codebase free-tier shape (marketing site + 3 auto-recovering FAST_NO_MODEL guest slots + heavy backend workers).

### 11.2 Parameterization is the whole difference (onprem == production)

```toml
# appliance.toml — the ONLY delta between AtomicSoul and any customer
guest_profiles = 10
domain = "atomicsoul.example"
[[node]]  name = "xbabe1"  endpoint = "…"  classes = ["gpu"]
[[node]]  name = "xbabe3"  endpoint = "…"  classes = ["gpu"]
[storage] default_durability = "standard"     # per-data override in product
[admin]   bootstrap_login = "…"
```

AtomicSoul **is** a customer install with production values (Codex SM-014, adopted). Configuration may change DNS, certs, node count/eligibility, guest count, quotas, storage/egress policy, and failure-domain labels — it **MUST NOT** select different product code. CI enforces it: a `parity-verify` job (ops repo) pulls the deployed digest set from AtomicSoul and byte-compares it against the signed release receipt of the shipped bundle; any drift fails the release lane. Supply chain unchanged: cosign-signed immutable digests, SBOM + provenance per image, installer verifies signatures before compose-up. Promotion to production remains owner-locked: two owner signatures over the exact `CloudReleaseSpec`, no self-signing, 24-hour canary soak, `formal_ga` stays false until the owner acts.

### 11.3 Joint projects on production

Enabled on AtomicSoul only after the tenancy isolation matrix (§12) is green on a staging appliance: multi-principal means Member data isolation holds, project budget ledgers debit correctly under concurrent collaborators, guest-profile priority holds under contention, and production-preempts-R&D holds when R&D is a shared project budget. Floating-profile count and per-project budget caps are admin-set at runtime.

---

## 12. Test program — one drill calendar

Every milestone exits through a **drill, not a demo**; each drill then becomes a permanent scheduled job on the hub. There is one calendar, not two parallel regimes.

- **Parity oracles (permanent CI):** `jope_embed` vs the frozen Python oracle (~1e-5), RedlineDB vs SQLite (`jeryu-storage-sqlite-oracle` exists), `StrictGpuSession` known-answer gate. Python stays a frozen parity oracle only.
- **Preemption drills:** against the existing `preemption-drill.kpi-schema.json` — push-cancel p99 ≤ 500 ms, empty-cgroup proof in `LeaseCancelAck`, quarantine + no-beneficiary-admission on no-ACK, checkpoint-requeue for `checkpoint_capable` victims, zero `Failed` (all `Preempted`). Re-baselined after the runner merge.
- **Mixed-lane saturation drill:** CI + live-coding + R&D + production inference on one node; production preempts R&D with the 0s hard-kill; Batch freezes under CPU/mem pressure; CI latency SLO holds.
- **Storage node-loss drills:** kill *k* nodes per declared durability profile; verify reconstruction + git-object integrity; verify jobs reading affected data requeue (not corrupt); bit-rot (flip shard bytes) → scrub detects + repairs with a receipt; key-rotation + topology-degradation behavior.
- **Shadow-master failover drill:** kill the hub primary; promote the shadow; verify RPO 0 on protected refs/immutable tags (byte-identical), RTO ≤ 15 min, protection active on the promoted master, in-flight PRs recoverable, no split-brain (old master fenced by quorum + gitd); then fail back via resync.
- **Tenancy isolation matrix:** anonymous guest cannot read any Member data; Member A cannot read Project B; admin sees Shared receipts but not Private artifacts; Member preempts anonymous for floating profiles with correct queue receipts; a revoked grant takes effect on next `authorize()` (no cached escalation); break-glass expiry actively closes capabilities.
- **Jankurai adversarial suite:** forged receipt (wrong key) rejected; replayed receipt (right key, wrong repo/SHA) rejected; stale receipt (policy fingerprint changed) rejected; **green check-run with no receipt → merge still refused**; a red-team agent session attempts `git worktree add`, `git config core.hooksPath`, force-push, out-of-grant write, `JANKURAI_SKIP_HOOKS=1` bypass, publish-with-hard-finding — **every attempt blocked with a denial receipt**; direct main push refused.
- **Supply-chain verification:** cosign verify all digests; floating-tag lint (no `:latest`, no unpinned refs); SBOM diff vs previous release; runtime probes on the running appliance (read-only rootfs, non-root, no docker.sock).
- **72-hour mixed-workload soak** (Codex §18, adopted): CI + interactive coding + ZYAL + ML + production + repair + guests + failover + node loss + migration; fails on leaked leases, orphaned workspaces, unbounded queues, unrepaired artifacts, divergent projections, lost receipts, authorization drift, or durability misreporting.

**Performance gates** (CI-enforced, on versioned reference hardware): warm lens switch p95 < 75 ms; runner event receipt-to-paint p95 < 100 ms; chat token gateway-to-paint p95 < 150 ms; first interactive LAN shell < 1.5 s; single-node hub idle RSS < 1 GiB; node idle RSS < 200 MiB; browser heap with 10k Run records < 250 MiB; leader recovery < 30 s with zero acknowledged-mutation loss (Codex §18 table, adopted).

---

## 13. Kill list

| Surface | Verdict |
|---|---|
| **jekko-web** (BFF + SPA) | **Delete.** Salvage exactly one thing → KNOWLEDGE/GRAPH tab (the @xyflow/elkjs reasoning graph, v1.1). |
| **jain-web SPA** | **Absorbed.** Feed/Composer/charts/socket/state move into the one SPA; the standalone Vite app retires; its axum chat/training routes fold into the hub binary. |
| **jain-tui**, **jekko-tui** | **Delete** as products (jekko chat runtime crates survive server-side). |
| **jeryu-tui** | **Demote** to a 5-lens (mission, release, queue, git, jankurai) feature-frozen break-glass console that reads the local RedlineDB directly. No new TUI features, ever. |
| **jeryu-web pages** | Dashboard/Work/PullRoom/Notifications/Audit → HOME + `g a`/`g n` drawers. Intelligence → KNOWLEDGE. Fleet/ToolFleet/Tools → one fleet drawer (`g f`). Repos/PR pages → CODE. Settings/Admin stay. 11 nav destinations → 3 lenses + drawers. |
| **Binaries** | Retire: `scqd`, `scq-node`, `scq-edge`, `jeryu-runnerd`, `guestd`, `jain-web-control`, `redlinedb-server` (embedded default), `routerd`. Their logic survives as hub/node libraries. |
| **jain-web-control (guest)** | Binary retired; lease semantics become a hub library; guests land in chat-zoom on a fresh Zero project (§2.6). |
| **jailgun apps** (fake-chatgpt, browser-adapter, dashboard) | Keep as internal sandbox tooling; the dashboard surfaces as a fleet-drawer tab. Not user-facing product. |
| **jain-jekko/**, 8 jekko scaffolds, tui-cutover, jain-index, jeryu-enterprise sso/tenancy/dr scaffolds | **Archive/delete** (§10.2), delta-diff first. |
| **jain-web-move42** | **Leaves the product family.** Stays as the standalone InnovationZero marketing site; no shared code; not in the manifest or release train. |
| **jankurai** | **Stays external.** The auditor audits this family and must not be governed by it (auditor independence). |

---

## 14. Milestones — the spine (M0–M8 + M-Ops)

Sequential at the program level; teams may prototype later work in isolation, but no later milestone claims exit by waiving an earlier authority/safety gate. Each exits through a drill from §12. All work from **M2 onward is dogfooded** through the product's own live-coding runners.

- **M0 — Name, freeze, inventory.** Product name (Jain) and shape numbers ratified; `repos.manifest.toml` (all three families) gains `state = active|absorbed|archived` + `absorbed_into`; splitctl refuses `pr-open` against non-active repos; archive list ratified (jain-jekko delta-diff produced before deletion); move42 leaves the family; frozen initial domain contracts + golden vectors; a future-major branch/release strategy isolated from 8.0.1; machine proof that no 8.0.1 metadata/evidence changed. *Exit: manifest is the single source of truth; zero commits land in doomed repos; 8.0.x lanes confirmed unaffected.*
- **M1 — Fused binaries + identity spine.** `jainhub`/`jainnode` composition roots stood up (forge + scheduler behind one process; node behind one process); Principal/Grant model + `authorize()` in jeryu-core; principal/grant/receipt tables; jain-web authenticates via forge session/PAT (owner-token + `JAIN_WEB_NO_AUTH` deleted); guest leases bind principals with two-class priority; joint Project = invitation-materialized grant bundle with a shared compute budget; provider keys server-side. *Exit: the same authorization result is proven across chat, data, git, knowledge, and Run APIs; a user can create and share a project; zero legacy auth paths reachable (grep-gate).*
- **M2 — Preemption drills + jankurai structural gates + DOGFOOD SWITCH.** ProofReceipt + proofbind signing with enrolled ed25519; `/proofs` endpoint + table; `verify_receipt`; `merge_pull` receipt gate; pre-receive mode for release/tags; `ExecPolicy` (banned argv incl. worktrees; landlock + `/grants` mounts; root-owned hooksPath) in the sandbox; host-mediated publish runs jankurai-guard with hard-finding refusal; preemption drill re-baselined. *Exit: no path to `refs/heads/main` without a valid receipt (adversarial suite green); KPI receipts green; from here, platform work is executed by agents in agentbridge cells under ExecPolicy.*
- **M3 — Enrollment + one-worker completion.** runnerd fleet/warm_pool/workcells folded into the node; RunnerClass as a workload-template execution backend; CI merge-queue as an Interactive/Batch submitter; agent cells Interactive; R&D Batch/Opportunistic; egress + sandbox-linux in the worker repo; node enrollment registry replaces static endpoints. *Exit: one binary per node; the two old fabrics deleted from active paths; node install (fresh machine → first job) < 10 min; family CI runs on the unified worker for 2 weeks.*
- **M4 — One web.** jeryu-web shell = survivor; jain-web zod3→4 + React 18→19 in place (gated by WS contract tests + recorded-frame replay); Chat dock absorbed; HOME on `repo_health` + AttentionItems; CODE consolidation; KNOWLEDGE (OCEAN/SOURCES/CONCEPTS tables, GRAPH deferred); keystroke view switching with persistent left pane + chat; unified TUI tokens; perf budget as a CI number. *Exit: one SPA serves all three lenses + chat; jain-web/jekko-web archived; team daily-drives it for 2 weeks.*
- **M5 — Distributed storage.** `jain-shard`: streaming, zstd, project-scoped dedup, XChaCha20 encryption, Reed-Solomon, HRW placement, repair/scrub WorkUnits, durability profiles, git/LFS integration, migration tooling. *Exit: destructive node-loss, bit-rot, topology-degradation, key-rotation, and repair testing at every advertised tolerance.*
- **M6 — Shadow master.** Designated follower; git sync/async replication; RedlineDB snapshot+op-log (then `redline-repl` after single-homing); hub-epoch fencing; owner-invoked promotion; break-glass TUI. *Exit: partition, crash-between-commit-and-apply, stale-follower, and repeated leader-failure drills prove no split-brain or acknowledged-mutation loss; RTO ≤ 15 min.*
- **M7 — Consolidation waves + appliance parity.** Absorption PRs per §10 (only after 8.0.x tags); redline single-homed (dual-lock retired); tombstone tags + manifest state flips; three-container appliance; `appliance.toml` parameterization; `parity-verify` CI; AtomicSoul redeployed FROM the customer artifact. *Exit: manifest shows the target small-repo map; a full family release ships from it; AtomicSoul == shipped artifact by digest diff; fresh-install < 30 min.*
- **M8 — Evolver + guest funnel + joint projects on production.** The Evolver loop (ocean epochs, search fan-outs, boil-down proposals, archival) on the unified ledger; provenance substrates unified; claim-on-signup guest funnel; joint projects live on AtomicSoul with the isolation matrix green under concurrent collaborators. *Exit: a dataset-only InnovationZero project and a repository-backed IP project use the same Run/knowledge/gate/receipt model; one real external collaborator on a joint project; 24-hour chaos soak with zero data-class violations.*
- **M-Ops (runs alongside M0–M2) — the real release orchestrator.** Implement the missing `release-candidate` spine (wave sequencing, identity binding, evidence aggregation) so many small repos release cheaply — the automation that makes the small-repo doctrine (§10) sustainable. *Exit: `deploy.sh` drives a real orchestrator end-to-end on a canary wave; release evidence is produced without manual per-repo steps.*

---

## 15. Reconciliation with `SUPER_MERGER_CODEX.md` (SM-001..SM-015)

Both specs describe the same product. This document is the engineering companion; the two agree on the architecture and differ on exactly three settled points (OD-1..OD-3). Disposition of Codex's accepted ADRs:

| ADR | Codex decision | This document |
|---|---|---|
| SM-001 | Jain is sole shell/gateway/brand | **Accept** (§0.2) |
| SM-002 | Jeryu accounts are identity authority | **Accept** (§2.1) |
| SM-003 | Jeryu sole Git object/ref/PR/check authority | **Accept** (§3.3, §8) |
| SM-004 | SmartCluster sole scheduler | **Accept** (§4.1) |
| SM-005 | ZYAL headless, scheduled through SmartCluster | **Accept** (§9) |
| SM-006 | Git worktrees prohibited without exception | **Accept + strengthen**: banned by *mechanism* four ways, not policy (§8.2) |
| SM-007 | Hosts, not agents, mutate refs / open PRs | **Accept** (§8.7) |
| SM-008 | One project-scoped distributed CAS for all artifact classes | **Accept + detail**: `jain-shard` extends SCQ v2's encrypted CAS with a stripe layer (§6) |
| SM-009 | Authorization precedes semantic retrieval | **Accept** (§2.5, §9.2) |
| SM-010 | Admin private access = 1-hour break-glass | **Accept** (§2.3) |
| SM-011 | Control HA uses a Raft command journal | **Amend → v2** (OD-3): epoch-fencing designated-follower for v1; Raft is the ≥3-controller-node upgrade (§7) |
| SM-012 | OpenRaft pinned behind an internal façade | **Amend → v2**: no new consensus dependency in v1; adopted when SM-011 lands in v2 (§7.1) |
| SM-013 | No dual-write migration | **Accept** (§10.4) |
| SM-014 | AtomicSoul and on-prem use identical digests | **Accept** (§11.2) |
| SM-015 | Rust owns contracts, generates TypeScript | **Accept** (§1) |

**The one added program decision** not in the Codex ADR list: the **small-repo doctrine** (OD-2, §10) — the owner's friction-driven consolidation rule, which sits between Codex's "no source-tree consolidation" non-goal and the design panel's 12-repo map, superseding both as the binding stance.

Where the two specs are read together, an implementation lane follows the **narrower** requirement, and any genuine conflict is resolved by a reviewed amendment + a superseding ADR before implementation (the Codex custody rule, honored). Neither document authorizes implementation.

---

## Appendix A — Asset → role map (every surviving component)

| Component (verified path) | Role in the merged product |
|---|---|
| `jain-smartcluster` (scqd, scq-node, jain-worker, scq-edge) | The scheduler spine → `jain-sched` + `jainnode` core; preemption law (§4) |
| `jain-smartcluster-core/src/fabric/node_protocol.rs` | The wire spine `jain-fabric` grows from |
| `jeryu-ci-runner` (runner-core, native/microvm/oci, sandbox-linux, runnerd, egress, ci-ir/compiler/governor) | Node-local execution planner + isolation; CI compilation (§4) |
| `jeryu-agentbridge` (driver, pty_driver, cli_registry) | Live-coding agent control → `jain-bridge` (§8) |
| `jeryu-core` (ForgeCore, gitd, storage, mirror, proof, readmodel, auth) | The forge + identity spine inside the hub (§2, §7, §8.6) |
| `jeryu-gitd/src/{refs.rs,protection.rs,hooks.rs,quarantine.rs}` | Ref service + the jankurai receipt gate insertion points (§8.6) |
| `jeryu-web/apps/web` (AppShell, cmdk, Monaco, xterm, tokens.css) | The one SPA shell (§3) |
| `jain-web/apps/web` (Sidebar, Feed, Composer, useSessionSocket, charts) | The chat dock + KNOWLEDGE/OCEAN charts (§3) |
| `jain-core/feat-core` (+ catboost/lightgbm/xgboost/jable, math, battle-gpu, starforge) | The AutoML/GPU engine (§1, §9) |
| `jain-agent` + `jain-zyal` / jekko `zyalc`, `zyal-supervisor` | Governed agent runbook + reasoning envelope (§8, §9) |
| jekko `jekko-provider`, `jekko-runtime`, `jnoccio-fusion` | Model gateway + provider runtime (§8.3) |
| jekko `agent-search` (16 providers, ProvenanceStore) | Knowledge search (§3.4, §9) |
| jekko `cogcore` (FSRS, Hebbian, consolidate) | Cognitive memory / boil-down (§9) |
| jekko `jekko-store/src/daemon/*` (run/task/pass/iteration/forever) | The Evolver's durable loop state (§9) |
| jekko `jekko-jailgun` | Air-gapped browser sandbox workload (§4.2, §13) |
| `jain-deploy` (appliance compose, guestd) | The one deployment artifact + guest lease library (§2.6, §11) |
| `jain-split-ops` (splitctl, host-CI, authority manifest) | The ops/release-orchestrator repo (§10, §14 M-Ops) |
| RedlineDB (`redline-core`, redline-central, db-shim) | The one embedded store; single-homed (§10) |
| jankurai (guard, proofbind, proofmark) | External auditor; receipts + gate (§8.6) |
| `jain-web-move42` | Leaves the family as the InnovationZero marketing site (§13) |

## Appendix B — Crate map (workspace shape)

**Hub (`jainhub`):** `jain-fabric` (protocol/domain), `jain-sched` (scheduler core), `jain-hub` (composition root), `jain-guest` (lease library), `jain-shard-map` (placement, new/small), `jain-shadow` (epoch fencing + git/RedlineDB replication, new; reuses jeryu-mirror), plus the forge libraries (jeryu-core/gitd/storage/proof/readmodel) and the model gateway (jnoccio-fusion).
**Node (`jainnode`):** `jain-node` (composition root), `jain-node-core` (fabric session + scq-edge replay), `jain-cells` (workcells/warm pool), `jain-runners` (native/microvm/oci behind one `Executor` trait), `jain-sandbox` (the single unsafe island, unchanged), `jain-bridge` (agent control), `jain-ci` (CI IR → CiJob), `jain-shard` (chunk store + Reed-Solomon + scrub, the one greenfield storage crate).

## Appendix C — Rejected alternatives (recorded for comparison)

- **Name JOVE** (design panel recommendation) — rejected for OD/owner-locked "Jain" (§0.2).
- **The 12-repo consolidation map** (redline, contracts, forge, runner, agents, knowledge, ml, ml-gpu, web, jailgun, deploy, ops) — rejected in favor of the small-repo doctrine (§10); retained here only as the shape the doctrine would collapse toward if release automation failed and coupling proved universal, which is not the expectation.
- **OpenRaft control HA in v1** (Codex SM-011/012) — deferred to v2 per OD-3 (§7).
- **Landlock write-grant regeneration mid-session** (one design's dynamic-grant approach) — rejected: landlock rulesets are add-only and cannot narrow without re-exec; `/grants` bind-mounts win (§8.4).
- **Client-side jankurai hooks as the gate** — rejected: `JANKURAI_SKIP_HOOKS=1` bypasses them; the ref-service receipt gate is the authority (§8.6).

---

*This specification defines a future-major target and the engineering path to it. It is not permission to mutate the 8.0.1 release candidate. Implementation proceeds only through separately claimed, repository-owned, reviewed work that remains isolated from and does not delay 8.0.1. It stands beside `SUPER_MERGER_CODEX.md`; §15 reconciles the two.*
