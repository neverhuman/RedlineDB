# SUPER MERGER — canonical engineering specification

**Program:** One Jain product from Jain, Jeryu, and ZYAL  
**Status:** Canonical program specification; implementation requires separately approved lanes  
**Target:** A future major release after Jain 8.0.1  
**Audience:** Product, architecture, security, storage, scheduler, UI, migration, release, and operations owners  
**Authority:** This file is the canonical engineering specification for the SUPER MERGER program

## 1. Status, scope, and normative language

SUPER MERGER creates one installable product and one Jain user experience while retaining
separate internal authorities for product experience, identity and Git, distributed execution,
and agent reasoning. It is a service merger, contract merger, and experience merger. It is not a
source-tree monolith.

The words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT, and MAY are normative.
An implementation is conformant only when every applicable MUST and SHALL statement is supported
by machine-verifiable evidence.

This file governs SUPER MERGER product and cross-service decisions. It does not supersede the
authority manifest, UPGRADE_CHAT coordination, repository-local ownership, AGENTS.md rules,
protected review lifecycle, or release evidence. A repository implementation may make a narrower
choice, but it cannot weaken a cross-service invariant here. Any necessary conflict is resolved by
a reviewed amendment to this file and a superseding ADR before implementation.

This specification is deliberately separate from the current 8.0.1 candidate:

- It MUST NOT change, delay, reinterpret, or become an exit gate for 8.0.1.
- It MUST NOT change candidate metadata, formal-GA metadata, the AtomicSoul push switch, rollback
  targets, immutable tags, branch protection, release evidence, Redline proof, or the reviewed
  release lifecycle.
- In particular, the current fail-closed values remain status=candidate, formal_ga=false,
  sagemaker=N/A, ATOMICSOUL_PUSH=0, and rollback target 7.0.6. This program cannot flip or reinterpret
  any of them.
- It does not authorize a source edit, branch, push, PR, merge, deployment, migration, destructive
  operation, or service change. Each implementation lane requires its own coordination claim,
  owning-repository rules, reviewed lifecycle, and exact-head proof.
- Every implementation branch MUST be explicitly designated for the future major and use the base
  approved by its repository owner. No compatibility shortcut may be backported into the 8.0.1
  candidate merely to prepare this program, and SUPER MERGER work MUST NOT serialize or delay the
  candidate.

### 1.1 Product assertion

Users install, open, authenticate to, navigate, and operate one product named Jain. Jain presents
one gateway, one shell, one project model, one chat experience, one Runner Board, one knowledge
experience, and one auditable action model. Jeryu and ZYAL are internal Rust services and engines.
They do not retain separate end-user products.

### 1.2 System authority

| System | Retained authority | Excluded or replaced |
|---|---|---|
| Jain | Sole product shell and brand; chat/session experience; projects and sharing; ML and research workflows; knowledge policy; SmartCluster integration | Token-derived single-user identity; duplicate session Git object/ref storage; unrestricted companion shell |
| Jeryu | Deployment accounts; Git objects and refs; repositories, branches, PRs, checks, issues, releases, workcells, CI compilation, code intelligence, governed agent branch transactions | Separate end-user frontend; authoritative runner scheduler; direct exposure as a separately branded product |
| SmartCluster | Admission, scheduling, reservations, leases, preemption, worker health, workload placement, and distributed artifact placement/repair | Any Jain, Jeryu, or ZYAL execution workload that bypasses it |
| ZYAL | Complex agent networks; advanced reasoning; promoted and negative memory; agent search/evidence; signed runbooks; sandbox primitives | Jekko TUI, Jekko Web, Jekko chat/session server, primary-repository mutation, and every Git-worktree workspace mode |

Jekko is a source and capability lineage, not a surviving product surface. Product-facing and API
terminology MUST use ZYAL for the extracted headless reasoning service.

### 1.3 Non-negotiable outcomes

1. Jain is the only user-facing shell and gateway.
2. The shell provides persistent left navigation, three rapid views, and persistent chat.
3. Every code, research, ML, agent, production, repair, and ad hoc execution workload is admitted
   through SmartCluster.
4. No user, agent, service, CI job, migration tool, test, or cleanup procedure creates or uses a
   Git worktree.
5. Code sessions use automatically removed standalone exact-SHA checkouts and namespaced,
   host-controlled Jeryu branches.
6. Uploads, Git/LFS, knowledge, models, checkpoints, and run outputs use one distributed artifact
   fabric.
7. AtomicSoul and on-premises installations consume the same signed release bundle and exact
   binary, image, migration, and toolchain digests.
8. Product and control-plane implementation remains Rust plus Vite, TypeScript, and React.
   Imported workloads may be polyglot only through signed, administrator-allowlisted toolchain
   images.
9. Deployment administrators can inspect deployment-shared resources and global operational
   telemetry. They may inspect private project content only through an audited break-glass grant
   that expires within one hour.

### 1.4 Conformance and document custody

Every numbered non-negotiable outcome, milestone deliverable and exit gate, acceptance journey,
performance gate, ADR decision, and implementation constraint is REQUIRED unless explicitly marked
informative. Before Milestone 2 implementation, a machine-readable conformance manifest assigns
each normative statement a stable SM-REQ identifier and maps it to:

- the governing ADR and contract version;
- owner repository and accountable role;
- milestone and prerequisite requirements;
- exact test or inspection procedure;
- versioned evidence schema and future-program evidence path;
- accepted repository head/tree, fixture digest, producer, result, and receipt digest.

No unmapped requirement can pass a milestone. The proof manifest binds the SHA-256 of this
specification and is stored outside all 8.0.1 release-evidence paths.

The split root is not itself a Git repository. Until a protected documentation authority is
designated, canonical revisions of this required root file use hash custody: each amendment claims
the documentation lane in UPGRADE_CHAT, records the prior and resulting content digests, explains
changed requirements/ADRs, and records reviewer disposition. Milestone 1 MUST establish the
approval roster, protected amendment ledger, and durable revision history without changing the
required root canonical path. A digest is recorded externally, never embedded self-referentially
inside this file.

## 2. Goals, non-goals, and success criteria

### 2.1 Goals

- Make simple dataset work and full repository-backed programs feel like different depths of the
  same product rather than different products.
- Give users one place to see work, blockers, compute, branches, artifacts, evidence, and decisions.
- Establish one authorization decision for chat, code, data, models, knowledge, and compute.
- Make all execution observable, governable, pausable where possible, and receipt-producing.
- Preserve Jeryu's Git transaction and CI semantics without duplicating them inside Jain.
- Preserve ZYAL's reasoning and memory capabilities without importing Jekko's competing UI or
  unsafe workspace behaviors.
- Support single-node installations while defining a truthful path to distributed durability and
  automatic control-plane failover.
- Make connected AtomicSoul and air-gapped on-premises deployments operationally equivalent.

### 2.2 Non-goals

- Combining all repositories into one source tree.
- Creating direct Jeryu-to-Jain product-crate dependencies.
- Replacing Jeryu as the authoritative Git-ref transaction engine.
- Replacing Redline as an embedded service projection store.
- Preserving old Jekko application surfaces.
- Supporting arbitrary host shells or arbitrary unsigned workload images.
- Claiming high availability from a two-controller topology.
- Allowing ordinary administrators to browse private project content.
- Providing a dual-write migration period.
- Changing the 8.0.1 candidate or release process.

### 2.3 Program-level definition of done

The program is done only when:

- all required journeys and adversarial tests in this specification pass;
- all milestone exit gates have exact-head evidence;
- the same signed release bundle passes AtomicSoul and air-gapped installation rehearsals;
- acknowledged control mutations survive leader failure exactly once;
- advertised storage tolerance survives destructive testing without content-hash loss;
- no execution workload covered by the Run definition bypasses SmartCluster;
- no Git worktree behavior remains reachable;
- legacy end-user surfaces are removed only after parity and rollback proofs pass; and
- a 72-hour mixed workload soak completes without leaked leases, orphaned workspaces, unbounded
  queues, or unrepaired retained artifacts.

## 3. Unified product domain

The following nouns are canonical across Rust contracts, JSON APIs, generated TypeScript, events,
UI labels, receipts, audit records, and documentation.

### 3.1 Deployment

A Deployment is one AtomicSoul or on-premises Jain installation. It owns deployment identity,
users, global policy, controller membership, workers, guest-seat capacity, shared-library review,
global telemetry, and release state.

### 3.2 Project

A Project is the collaboration, policy, compute, storage, and knowledge boundary. A project may
contain:

- datasets and uploaded files;
- hidden micro-Git history;
- visible Jeryu repositories;
- project-scoped models and reports;
- knowledge collections and memory capsules;
- sessions, runs, attention items, and campaigns;
- membership, resource grants, budgets, and production reservations.

A project is private by default. Authorization MUST be evaluated within the project before data
is eligible for retrieval, scheduling, mounting, rendering, export, or semantic ranking.

### 3.3 Session

A Session is a chat stream within one project. It may bind datasets, knowledge collections,
resources, and one or more exact Git refs. A session is not the Git authority and does not own a
second Git object store.

Multiple sessions may start from the same exact base OID. Each receives an isolated namespaced
branch and standalone checkout.

### 3.4 Resource

A Resource is an addressable repository/ref, dataset, artifact, model, report, or knowledge
collection. ResourceGrant refines project membership without copying the resource. A grant carries
the grantee, allowed actions, scope, optional expiry, policy version, and issuing actor.

### 3.5 Run

A Run represents every admitted execution, including:

- CI and checks;
- governed live agent coding;
- interactive human terminals;
- ZYAL networks and search;
- dataset profiling and feature generation;
- training, evaluation, and inference;
- reports and research extraction;
- production services and allocations;
- artifact verification and storage repair.

Ordinary bounded control-plane request handling is not a Run. Any task that consumes schedulable
worker compute, executes project code, touches mounted project capabilities, or can outlive an API
request MUST be a Run admitted through SmartCluster.

### 3.6 AttentionItem

AttentionItem is the normalized actionable blocker. Kinds include failed checks, release gates,
stalled or unhealthy jobs, requested approvals, audit findings, contradictory knowledge, exhausted
budgets, degraded storage, lost worker leases, migration mismatches, and quorum risk.

Every item has severity, owner, affected resources, evidence links, allowed actions, creation and
update sequence, and terminal disposition. A warning without an actionable disposition is
telemetry, not an AttentionItem.

### 3.7 Campaign

A Campaign represents continuing evolution. It contains:

- goal and success measures;
- cadence and next activation;
- execution and cost budget;
- stop and pause conditions;
- promotion gates;
- attached data, repositories, models, and knowledge;
- archive and reopen policy.

InnovationZero is a preset Campaign created from a dataset upload. Full IP programs use the same
object with additional repositories, agent networks, production workloads, and recurring schedules.

### 3.8 Quick upload behavior

A quick dataset upload MUST:

1. create a private project when no destination project is selected;
2. create a hidden Jeryu micro-repository for provenance and evolution history;
3. stream content into the project artifact fabric;
4. bind the upload hash, manifest, actor, and repository checkpoint;
5. expose a simple dataset experience without requiring the user to understand Git.

More advanced users may attach visible repositories. Hidden and visible forms use the same Jeryu
Git engine, storage fabric, authorization model, and run model.

### 3.9 Production service

A ProductionService is durable desired state for a project-owned serving workload. It binds a
signed immutable Revision, replica and availability policy, production reservation, routing
policy, health contract, rollout/rollback gates, models/artifacts, and authorization.

Each replica activation is a SmartCluster Run using jain.production_service. A service controller
proposes desired replica count, reservation use, health disposition, and routing promotion through
the committed command journal; SmartCluster alone chooses placement. Revision changes use staged
health checks and explicit promotion. Interactive borrowers leave reserved capacity through the
pressure ladder before required production replicas are admitted.

## 4. Jain experience

### 4.1 Persistent shell

The desktop shell has three persistent regions:

~~~text
┌──────────────────────┬──────────────────────────────────────────┬─────────────────────┐
│ Projects / sessions  │ Overview · Code · Knowledge              │ Persistent chat     │
│ Context-sensitive    │ Main actionable canvas                   │ Context + composer  │
│ drill-down tree      │ Runner board, code, graph, or artifact   │ Plans and receipts  │
└──────────────────────┴──────────────────────────────────────────┴─────────────────────┘
~~~

- The left pane always starts with projects and reveals relevant sessions, repositories,
  campaigns, runs, and collections beneath the selected project and view.
- The chat rail remains mounted in Overview, Code, and Knowledge. It can expand into the main
  canvas or collapse without losing draft, context, generation, tool receipts, or scroll state.
- The central route is always deep-linkable. Project, selected resource, filters, tabs, chat
  context, and relevant scroll anchors survive refresh and restore per user.
- Mobile and narrow layouts may stack regions, but MUST preserve the same navigation hierarchy,
  active chat state, and deep links.

Keyboard navigation:

| Command | Action |
|---|---|
| g then o | Open Overview |
| g then c | Open Code/Operations |
| g then k | Open Knowledge |
| Cmd/Ctrl+K | Open the command palette |
| Cmd/Ctrl+J | Focus chat |

Landing behavior:

- deployment admins land on the deployment-wide Runner Board;
- authenticated non-admin users land on their last project and view;
- guests land directly in the project shared by the guest link;
- selecting a project opens its Runner Board;
- selecting a repository, branch, run, blocker, artifact, or knowledge node opens a focused
  cockpit without discarding chat context.

The visual language MUST be high contrast, keyboard accessible, responsive to reduced-motion and
contrast preferences, and operable without a pointer. Color MUST NOT be the only carrier of state.

### 4.2 Overview

Overview summarizes project purpose, participants, active campaigns, recent sessions, production
health, budgets, storage durability, important artifacts, knowledge changes, and AttentionItems.
It prioritizes decisions and outcomes rather than duplicating the full operations surface.

### 4.3 Runner Board

The Runner Board is the default project canvas in Code/Operations and the primary admin landing
page. It shows all Runs, not only CI.

Required workload coverage:

- active agent branches and live sessions;
- checks and CI;
- ZYAL phases and searches;
- experiments, profiling, features, training, and inference;
- production allocations;
- checkpointing, paused, and requeued work;
- storage verification and repair;
- queued, stalled, failed, canceled, and completed work.

Each visible run exposes, where applicable:

- queue position, priority class, reservation, and scheduling reason;
- worker, failure domain, trust class, and sandbox backend;
- CPU, RAM, GPU, VRAM, and storage consumption;
- elapsed time, checkpoint age, and estimated remaining budget;
- artifacts, logs, branch, base/commit OID, PR, and knowledge links;
- current blocker, gate, next action, and immutable receipts.

Lanes are running, queued, checkpointing/paused, production-reserved, and needs-attention.
Completed work is available through filters and history rather than an unbounded live lane.

Deployment-wide admin mode additionally shows worker health, utilization, storage placement,
quorum, controller eligibility, guest-seat usage, per-project budgets, and durability shortfalls.
Operational aggregation MUST NOT reveal private project content.

Actions include pause, resume, checkpoint, reprioritize, retry, cancel, open live session, inspect
artifacts, open branch/PR, and resolve a gate. Cancel, kill, break-glass, quota override, and
destructive storage actions require explicit confirmation. Every attempt, including refusal,
enters the receipt rules in Section 13.3; only a quorum-committed state action is represented as a
global ActionReceipt.

Production capacity MUST visually distinguish capacity actively reserved for production from
reserved capacity temporarily borrowed by interactive work.

### 4.4 Code/Operations

Code embeds Jeryu capabilities in the Jain shell:

- repository browser and code search;
- branches, commits, diffs, PRs, issues, and releases;
- check and audit results;
- code intelligence and navigation;
- governed live terminals and agent session controls;
- exact base, session branch, workspace state, gates, and host-captured commit history.

Jeryu does not render a separate application chrome. A project selection opens the Runner Board;
a repository or branch selection drills into code.

Agents never push. The host records commits, advances the session branch using compare-and-swap,
runs required gates, and opens or updates the protected PR.

### 4.5 Knowledge

Knowledge exposes ZYAL reasoning and memory through Jain-native components:

- evidence graph with citations and provenance;
- virtualized searchable artifact and memory lists;
- timeline of hypotheses, decisions, experiments, features, and models;
- negative lessons and rejected approaches;
- contradiction and supersession edges;
- search-provider receipts, hashes, taint state, quarantine state, and extraction provenance;
- campaign winners, rejected ideas, archives, and reopen history.

The graph MUST aggregate by semantic and structural level of detail. It MUST support at least
100,000 stored nodes while rendering no more than 2,000 visual nodes. Lists MUST be virtualized.

Memory promotion states are:

1. scratch — attempt-local and freely expirable;
2. run — retained with the Run;
3. project — reusable within the project authorization boundary;
4. shared_library — deployment-scoped and administrator-reviewed.

Promotion to shared_library MUST copy only explicitly approved material whose content policy
permits deployment sharing. It MUST NOT make private project content discoverable. Retrieval
filters authorization before semantic ranking. Existing Jekko Global memory migrates to
shared_library_pending, never directly to shared_library.

Recomputable scratch artifacts may expire under project policy. Promoted conclusions, citations,
negative knowledge, decisions, and the receipts supporting promotion remain retained.

## 5. Deployment architecture

### 5.1 One release bundle, two roles

The signed Jain release bundle installs two roles:

**jain-control**

- supervisor and lifecycle manager;
- HTTPS gateway and Jain BFF/UI;
- project, session, resource-grant, campaign, and knowledge policy authority;
- embedded Jeryu API and Git endpoints;
- SmartCluster controller and edge API;
- deterministic zyald runbook compiler, planner, and coordinator;
- control consensus member when enabled;
- immutable audit journal;
- independent local Redline/Jeryu projections.

**jain-worker**

- outbound SmartCluster node agent;
- workload executors and sandbox backends;
- artifact transfer, placement, verification, and repair service;
- Jeryu workcell adapter;
- ZYAL signed-phase executor;
- Jain data, research, ML, report, and inference executors.

Same-host internal services listen on Unix sockets or mutually authenticated loopback endpoints.
Multi-controller deployments additionally expose a dedicated mutually authenticated
controller-membership transport for Raft, snapshots, and projection-digest exchange. That endpoint
is reachable only on the deployment's private or authenticated overlay, uses controller-specific
identities and audiences, and is never routed through the public gateway.

Publicly reachable surfaces are limited to:

- the Jain HTTPS gateway;
- Jeryu Git HTTPS and optional SSH under Jain-owned routing and identity;
- outbound worker-to-controller mTLS connections.

No internal service may independently expose an unauthenticated or separately branded frontend.
Artifact chunk, shard, checkpoint, and repair traffic is multiplexed over bounded, flow-controlled,
resumable streams on the worker's outbound mTLS connection. The controller routes capability-bound
streams between workers without treating relayed bytes as controller-local authoritative storage.
Frame, window, concurrency, timeout, and backpressure maxima are contract fields. Direct
worker-to-worker listeners require a future security ADR and are not part of this design.

Relay traffic runs in a separately resource-controlled process or execution pool with bounded
memory, descriptors, disk spill, CPU, and concurrency. Per-project quotas, fair queuing, and
control-plane-reserved bandwidth ensure artifact upload, download, repair, or a slow receiver
cannot starve Raft heartbeats/snapshots, authentication, API admission, receipts, or leader
recovery. Saturation and adversarial backpressure tests are release gates.

### 5.2 Stable service boundaries

Jain, Jeryu, SmartCluster, and ZYAL communicate through versioned local APIs and signed,
hash-bound workload bundles. Direct Jain-to-Jeryu and Jeryu-to-Jain crate dependencies are
prohibited. No service may gain a source-level dependency that inverts ownership or directly
mutate another service's projection database.

Each adapter owns:

- version negotiation;
- request/response translation;
- identity and authorization context propagation;
- idempotency and retry semantics;
- bounded timeouts and cancellation;
- receipt verification;
- projection lag reporting.

Internal APIs remain compatible for at least the supported upgrade window. Unknown required fields,
unknown template versions, invalid signatures, stale fence tokens, and unsupported discriminants
fail closed.

### 5.3 Command-line surface

The supported operator CLI is:

~~~text
jain init
jain status
jain node join <controller> --ticket-stdin
jain node bootstrap <host> --user <user>
jain node revoke <node>
jain controller join <leader> --ticket-stdin
jain controller remove <controller-id>
jain backup
jain restore
jain upgrade
~~~

Commands MUST use typed argv internally, support non-interactive automation where safe, return
stable exit codes, redact secrets, and emit a machine-readable receipt. Destructive or
availability-affecting operations require an explicit confirmation or an exact non-interactive authorization
flag documented for that operation.

Ticket bytes are read from protected standard input; a programmatic API may use an already-open
protected file descriptor. Literal ticket values in argv, environment variables, URLs, or files
with broader-than-owner permissions are refused. The admin UI displays a join invocation that
names the safe source and supplies the one-use secret only through the corresponding protected
prompt or descriptor.

### 5.4 Node enrollment

Normal enrollment:

1. an admin generates a one-use join command in Jain;
2. the ticket binds deployment, expiry, intended capability/trust policy, and a single redemption;
3. the worker validates the controller identity and establishes outbound mTLS;
4. the controller atomically consumes the ticket and issues the node identity;
5. both sides record non-secret enrollment receipts.

Optional SSH bootstrap:

1. the operator supplies the host and remote user;
2. Jain pins and displays the host fingerprint for approval;
3. Jain copies the already signed worker bundle;
4. Jain installs the service using an exact, receipt-bound argv;
5. one-use credentials are discarded immediately;
6. the installed worker completes the normal outbound enrollment.

Passwords, private keys, bearer tokens, join tickets, and recovered secrets MUST NOT enter logs,
databases, receipts, shell history, process titles, or telemetry. Node identities and controller
certificates rotate. An admin can fence or revoke a node centrally.

A worker advertises CPU, RAM, GPU and VRAM, storage, failure-domain labels, sandbox backends,
cached artifact hashes, trust class, and capability versions. Claims are validated where possible
and treated as untrusted placement inputs until attested by policy. A worker advertisement can
never grant controller eligibility or consensus membership.

Controller eligibility is an administrator-approved property of a jain-control identity. Join and
remove operations use one-use protected tickets, controller-specific certificates, stable member
IDs, and OpenRaft joint-consensus membership changes. Removing a member rotates or revokes its
transport and unseal access. A promoted worker requires an explicit jain-control installation and
controller enrollment; self-advertisement is insufficient.

### 5.5 Deployment parity

AtomicSoul and on-premises installations use identical:

- signed bundle manifest;
- controller and worker binaries;
- OCI and toolchain image digests;
- schema migrations;
- default runtime behavior;
- contract and compatibility versions.

Configuration may change DNS, certificates, node count, controller eligibility, guest-seat count,
quotas, storage policy, egress policy, offline registry location, and failure-domain labels. It
MUST NOT select different product code.

## 6. Identity, authorization, collaboration, and guests

### 6.1 Identity authority

Jeryu accounts are the deployment identity authority. Jain's token-hash single-user identity is
replaced. The Jain gateway performs authentication orchestration and propagates a signed actor
context; services still make authorization decisions from canonical membership, grants, and policy
versions rather than trusting browser claims.

Deployment roles:

- admin;
- user.

Project roles:

- owner;
- collaborator;
- viewer.

Deployment admin is not an implicit project owner and does not imply access to private content.

Authentication and authorization have distinct authorities:

1. Jeryu authenticates the deployment account and enforces account, credential, PAT, SSH-key,
   lockout, and session state.
2. The Jain policy-decision point evaluates project role, ResourceGrant, compute/storage policy,
   guest or break-glass state, and the committed policy sequence.
3. Jeryu applies repository ACL, branch protection, hook, check, and Git transaction constraints.
4. Effective authorization is the intersection of all applicable decisions; any deny wins.

Jain owns one versioned AuthorizationDecision contract, evaluator, and adversarial conformance
corpus. Other services may impose narrower domain constraints but MUST NOT independently broaden a
decision. Direct Git HTTPS, Git SSH, PAT, UI, REST, event replay, mount, search, and worker paths
must call the decision point or validate a short-lived signed decision capability at the same
committed policy sequence. A lagging or unreachable decision point fails closed for mutations and
sensitive content reads.

Signed actor/decision contexts bind issuer, audience, actor, authentication strength, delegation
chain, deployment, project/resource/actions, policy sequence and digest, request or Run/attempt,
expiry, nonce, and key ID. They are audience-restricted, replay-protected, rotated, and revoked by
policy sequence or fence change. Browser claims alone are never sufficient.

### 6.2 Project role semantics

Owners manage members, project grants, project budgets, production reservations, sharing,
retention, and risk policy within deployment limits.

Collaborators may use permitted data, submit Runs, contribute session branches, share project
compute, and mutate resources covered by grants.

Viewers may read permitted content and observe permitted Runs. They may not mutate resources or
consume compute unless an explicit ResourceGrant permits the action.

ResourceGrant may narrow or extend role defaults for code, datasets, models, knowledge, reports,
and compute. Deny rules and expiry are evaluated before allow rules. Authorization results bind the
policy version and are revalidated at sensitive action boundaries, mounts, promotion, and resume.

### 6.3 Floating guests

Guests are leases, not permanent operating-system or deployment database accounts.

- An admin configures the maximum number of concurrent guest leases.
- A guest link is project-scoped and binds expiry, access profile, data grants, compute budget,
  and maximum concurrency.
- A lease expires after 15 minutes idle and refreshes while a validated connection is active.
- Authenticated users are admitted before new guests.
- Guest background work may checkpoint and requeue when capacity is required by policy.
- Active non-checkpointable work is not silently destroyed; it follows the explicit pressure and
  kill policy and produces lost-work receipts if termination is authorized.

Guests cannot:

- create PATs or SSH keys;
- invite members or create guest links;
- change storage risk or retention policy;
- publish shared knowledge;
- create production reservations;
- use break-glass;
- request undeclared egress or privileged sandbox capabilities.

Authoritative guest activity is an authenticated foreground interaction, acknowledged chat/event
heartbeat, or permitted active interactive Run—not merely an open socket or background tab.
Heartbeats are server-timed, rate-bounded, and deduplicated across tabs. Disconnect receives a
bounded reconnect grace shorter than the idle window. Link redemption creates a pseudonymous guest
actor ID for audit; links and individual leases can be revoked immediately. Expiry or revocation
increments the policy sequence, fences compute, invalidates decision capabilities and event
cursors, closes mounts and interactive channels, and prevents cached server content from being
served on a new request. Fake-clock tests define boundary behavior.

### 6.4 Break-glass

Private-content access by an admin requires a BreakGlassGrant:

- a specific project and content scope;
- a recorded human reason;
- strong reauthentication;
- a maximum one-hour expiry;
- an immutable grant and access trail;
- immediate notification to project owners;
- automatic post-expiry denial.

Break-glass does not grant source mutation, membership changes, key export, shared-library
publication, or policy bypass. Renewal is a new grant with a new reason and notification.
Expiry or revocation uses the same policy-sequence invalidation path as a guest lease and actively
closes capabilities, mounts, event streams, and private-content sessions rather than relying only
on the next API request.

## 7. Universal workload fabric

### 7.1 Sole scheduling authority

SmartCluster is the only authority for admission, queuing, placement, scheduling, reservations,
leases, preemption, checkpoint coordination, worker health, attempt fencing, and restart adoption.

Jeryu retains CI compilation, trust tiers, sandbox/workcell semantics, and agent branch management.
Its runner registry and prior scheduler become non-authoritative projections of SmartCluster
events. Jain and ZYAL likewise submit Runs rather than selecting workers directly.

The enforcement stack has one direction of authority: ZYAL declares tools and capabilities in a
signed plan; Jeryu defines code, Git, trust-tier, and workcell policy; SmartCluster admits the
Run and owns attempt/lease/fence state; the jain-worker sandbox enforces host isolation, mounts,
devices, argv, and egress. A Jeryu workcell claim is a subordinate projection keyed to the active
SmartCluster attempt and fence token. It cannot maintain an independent liveness lease, extend an
attempt, or publish after SmartCluster fencing.

### 7.2 Workload templates

RunSpec references a versioned WorkloadTemplateId, never a shell string. Initial templates are:

- jeryu.ci;
- jeryu.agent_session;
- jain.dataset_profile;
- jain.feature_search;
- jain.training;
- jain.inference;
- jain.report;
- zyal.network;
- zyal.search;
- system.storage_repair.

The minimum registry additionally reserves explicit templates for jeryu.human_session,
jain.evaluation, jain.production_service, system.artifact_verify, and
system.control_maintenance. An ad hoc request must still select a signed, approved concrete
template and image; there is no generic shell template.

Each template version binds:

- executable or image digests;
- typed argv schema;
- allowed inputs, outputs, mounts, devices, and egress;
- sandbox and trust requirements;
- checkpoint protocol;
- minimum worker capabilities;
- default resource bounds and maximum expansion;
- receipt schema and promotion gates.

Template resolution is administrator-governed and signature-checked. RunSpec cannot replace the
template executable with arbitrary shell text.

A generated executable-entrypoint inventory maps every Run kind, CLI/UI action, service adapter,
worker executable, maintenance task, and legacy compatibility path to a template version,
RunSpec-to-JobSpec compiler, policy owner, and admission receipt. Build and runtime checks refuse
an executable entry point that is absent or ambiguous in this registry.

Existing Postcard discriminants and V1 jobs MUST retain their numeric and binary meaning. New
variants append; existing enums are never reordered. A compiler converts the unified RunSpec into
SmartCluster JobSpec without losing actor, project, grants, budget, template version, or receipt
bindings.

Production is appended as a new wire variant to the existing PriorityClass contract, then assigned
the logical rank between system and interactive. Existing System, Interactive, Batch, and
Opportunistic discriminants do not move. Likewise, legacy AdministratorArgv remains decodable only
where V1 compatibility requires it; /api/v2 and every new adapter MUST resolve an approved
WorkloadTemplateId and cannot use AdministratorArgv as an escape hatch.

### 7.3 Priority and reservations

Scheduler priority, highest first:

1. system;
2. production;
3. interactive;
4. batch;
5. opportunistic.

Production allocations are reserved and non-preemptible by default. Interactive work may borrow
idle reserved resources, but the borrow is visible and the workload MUST support the configured
checkpoint/pause response before placement there. Production reclaim is not reported as failure.

Interactive work may preempt compatible batch and opportunistic attempts. Project budgets,
deployment safety reserves, and fairness apply within priority classes; priority alone does not
authorize a resource or bypass a quota.

### 7.4 Pressure ladder

SmartCluster applies pressure in this order:

1. stop admitting lower-priority work;
2. freeze or pause a compatible workload;
3. request a checkpoint with a deadline;
4. persist and verify the checkpoint in distributed storage;
5. close and fence the old attempt, then requeue the Run from the verified checkpoint so the next
   placement receives a new attempt ID and fence token;
6. kill only after the grace period when policy permits.

Kill records the reason, grace interval, last checkpoint, lost-work estimate, authorization,
retry decision, and receipt. A stale attempt cannot publish output, advance a branch, renew a
lease, or claim success after its fence token changes.

### 7.5 Run state

The canonical state machine distinguishes Run from attempt:

~~~text
draft → admitted → queued → assigned → starting → running
                                      ↘ blocked
running → checkpointing → paused → queued
running → succeeded | failed | canceled | lost
failed | lost → retrying → queued
~~~

A Run has stable identity across attempts. Each attempt has its own worker lease, fence token,
timestamps, logs, outputs, and terminal receipt. Transitions are journaled, idempotent, monotonic
by global sequence, and safe to replay.

The frozen transition matrix defines every source/target edge, allowed actor, preconditions,
authorization sequence, fence behavior, receipt, timeout, and terminality. It includes gate
resolution from blocked, cancellation from every nonterminal state, pause/checkpoint refusal that
leaves the Run safely running, retry exhaustion, deadline/budget termination, worker loss, and
parent/child ZYAL aggregation. An absent transition fails closed.

## 8. Governed live coding and Git

### 8.1 One code-session path

Every human or agent code session follows this sequence:

1. Jeryu resolves the repository, authorization, and exact base OID.
2. The worker creates an automatically removed standalone sandbox directory.
3. The repository is cloned from canonical Jeryu with git clone --no-local under an isolated
   HOME and closed Git configuration, then verified against a server-bound exact-OID/object-closure
   manifest. File protocol, alternates, local object sharing, hooks, credential helpers, automatic
   submodule recursion, and smudge/process filters are disabled. Approved submodules and LFS are
   resolved separately through signed resource manifests.
4. Jeryu creates agents/<agent-id>/sessions/<run-id> for an agent or
   users/<actor-id>/sessions/<run-id> for a human.
5. SmartCluster mounts only authorized code, data, tool, output, and network capabilities.
6. Tool calls use typed program/argv requests; agents receive no raw host shell.
7. Policy refuses Git worktrees, unsafe host paths, broad destructive commands, direct push,
   hook bypass, undeclared network access, symlink escape, and writes outside workspace/output.
8. Jankurai and repository precommit rules run before host commit capture.
9. The host captures the commit and advances the session branch using compare-and-swap.
10. Protected PR checks rerun the required gates; the host opens or updates the PR.
11. The standalone checkout is removed by a bounded, versioned deadline after terminal,
    abandoned, or fenced state. Recovery retains verified checkpoints, patches, commits, logs,
    and artifacts—never an indefinitely live checkout. Startup and periodic sweepers delete
    expired physical sandboxes and emit deletion or actionable refusal receipts.

Concurrent sessions from the same base use independent branches and physical checkout directories.
No linked object store, alternates file, or worktree registration is permitted.

### 8.2 Agent and human boundaries

Agents do not receive the current unrestricted companion Bash session, host credentials, Git push
credentials, hook-bypass flags, or an untyped command tunnel.

Human terminals are also sandboxed and capability-bound. Policy may permit a broader signed
toolchain than an agent receives, but never a host shell or direct primary-checkout mutation.

Only the host may:

- create or advance session refs;
- capture commits;
- push to Jeryu;
- open or update PRs;
- post checks and receipts;
- promote outputs into authoritative resources.

### 8.3 ZYAL workspace compatibility

The following ZYAL/Jekko modes are removed and cannot be accepted as aliases:

- IsolatedWorktree;
- WorkspaceKind::Worktree;
- GitWorktree;
- PrimaryRepo write mode.

Supported modes are:

- Scratch;
- StandaloneCheckout;
- SessionBranch;
- OutputOnly.

Direct primary-repository mutation is not a ZYAL capability. Compatibility import MUST fail closed
when an old runbook requests a removed workspace mode.

## 9. ZYAL reasoning, memory, search, and evolution

### 9.1 Headless zyald

zyald is an internal headless service. It compiles a signed, hash-bound runbook into a DAG of
phases, roles, dependencies, budgets, tools, data grants, and exit gates.

The compiled plan binds:

- source runbook hash and compiler version;
- project, actor, authorization snapshot, and policy version;
- phase graph and dependency hashes;
- role and model/tool identities;
- compute, token, time, cost, and storage budgets;
- input grants and output namespaces;
- host-owned gates and approval points;
- allowed search providers and egress;
- cancellation and checkpoint behavior.

Deterministic compile, planning, dependency release, and coordination run inside jain-control and
are journal/projection state. Every executable phase is a child SmartCluster Run. Attempts are
retries or placements of that same phase Run, never distinct phases. jain-worker executes only
signed compiled phases. zyald does not maintain a competing worker pool.

### 9.2 Knowledge ledger

Reasoning artifacts, evidence, edges, lanes, memory capsules, receipts, hypotheses, decisions, and
promotion events are stored in the project knowledge ledger and bulk artifact fabric.

The ledger preserves episodic, semantic, procedural, and negative memory. Nodes are content-bound;
edges are typed and versioned. Contradiction and supersession do not erase prior claims.

### 9.3 Host-owned gates

Tests, parity, Jankurai, evidence, budget, approval, authorization, and repository-graph freshness
are host-owned. An agent or runbook cannot relax, omit, reinterpret, self-approve, or reorder them.

ZYAL may propose code, features, models, tools, knowledge promotion, or a new Campaign. Promotion
always passes the owning Jain or Jeryu gate.

### 9.4 Search and hostile content

Search and extraction use administrator-approved egress. Results retain provider, request and
response hashes, retrieval time, citations, extraction version, taint state, quarantine state,
and prompt-injection findings.

Untrusted retrieved content remains data. It cannot grant tools, change the runbook, broaden
authorization, disable a gate, or become executable instructions. Quarantined content is excluded
from automatic memory promotion.

## 10. Distributed artifact and storage fabric

### 10.1 Scope and authority

The fabric stores uploads, datasets, Git pack objects, LFS objects, knowledge artifacts, models,
checkpoints, reports, logs, run outputs, and repair evidence. Jeryu remains authoritative for Git
ref transactions; the fabric stores Git content and verifies reachability.

Controller-local artifact storage is not authoritative after migration.

Each Jeryu repository has one home_project_id and storage-security domain. Another project either
receives an explicit ResourceGrant to that home resource or creates a verified fork/copy into its
own domain; attaching one repository by reference to multiple project key domains is prohibited.
Hidden upload micro-repositories commit signed dataset manifests, provenance, and CAS pointers,
not duplicate dataset bytes inside Git history.

Canonical Git objects are keyed and verified by the repository's declared SHA-1 or SHA-256 object
format and stored through the CAS. Packfiles, indexes, and transfer bundles may also use the fabric
but are derived, generation-bound artifacts; repacking cannot change canonical object identity or
ref reachability, and obsolete packs follow Jeryu-aware garbage collection.

### 10.2 Data path

Large content is:

1. streamed through content-defined chunking averaging 4 MiB;
2. identified and deduplicated within the project before encryption;
3. compressed with Zstandard level 3;
4. encrypted with the project data key using XChaCha20-Poly1305;
5. erasure-coded over ciphertext when topology permits;
6. placed across distinct node, rack, or site failure domains;
7. verified with per-shard checksums and end-to-end content hashes.

Each project has separate deduplication and key domains. The dedup lookup ID is
HMAC-SHA-256(project-dedup-key, uncompressed-chunk); it is never exposed across projects. A new
unique chunk receives a random 256-bit data-encryption key and random 192-bit nonce. The compressed
bytes are encrypted with XChaCha20-Poly1305, and the data key is wrapped by the versioned project
key-encryption key. A duplicate reuses the already verified project-local encrypted chunk rather
than deterministically encrypting plaintext again.

Authenticated additional data binds deployment storage domain, project, dedup ID, algorithms and
versions, and uncompressed and compressed lengths. Object-specific ordinal and order are bound by
the authenticated hierarchical manifest, not reusable chunk ciphertext. Readers verify the AEAD
tag, declared compressed bound, decompression output bound, chunk hash, manifest order, and final
object hash before publication. No key/nonce pair is ever reused.

Project key rotation rewraps per-chunk data keys without rewriting bulk ciphertext unless an
algorithm migration requires it. Cryptographic erasure destroys every live wrap of project key
material in deployment-controlled controllers, online snapshots, managed backups, and managed
offline targets after retention permits it, and propagates signed tombstones. Independently
exported or immutable media remains recoverable until its declared expiry or verified physical
destruction; Jain reports that residual recovery window in the UI and audit trail. Retained audit
records contain non-secret commitments, not recoverable key material.

Manifests bind content hash, ordered chunks, compression, encryption metadata, coding parameters,
placement receipts, retention, project, policy, and repair history. Small objects and control
metadata use full replicas rather than erasure coding.

Streamed manifests and bounded transfer windows replace the current 512 MiB object and 1 GiB job
ceilings. Memory use MUST remain bounded independently of object size.

Promotion to shared_library copies approved bytes into a separate deployment-library
deduplication, encryption, retention, and authorization domain. Publication verifies and
re-encrypts the complete promoted object before committing the library manifest. The library copy
survives source-project deletion or key erasure, while provenance visible outside the source
project contains only administrator-reviewed, non-private fields.

### 10.3 User-facing risk policies

| Policy | Promise |
|---|---|
| scratch | Recreatable and TTL-bound; no node-loss guarantee |
| balanced | Default project data policy; survive one node loss |
| protected | Survive two node losses |
| critical | Survive three node losses and require a configured offline/export target |

The table promises node-loss tolerance. For a large object with node-loss tolerance f, use
Reed-Solomon 4+f when at least 4+f distinct eligible nodes exist. Otherwise use f+1 full replicas
when possible. If neither is possible, the requested policy is not achieved. Rack- and site-loss
tolerance are calculated and reported separately from actual shard placement; a node-level promise
never implies rack or site survival.

Failure-domain labels require administrator attestation and topology validation. A worker's
self-reported label cannot establish durability. Placement for a claimed rack or site tolerance
must put the required shards or replicas in independent domains at that exact level.

Every project storage surface MUST continuously display requested and achieved node, rack, site,
and offline durability, degradation reason, repair backlog, and offline-target freshness.
Deployment-wide admin mode aggregates the same without revealing private content. The system MUST
NOT silently claim a risk level that current topology, placement, repair backlog, or offline-target
freshness cannot satisfy.

Git refs, project membership, audit logs, control snapshots, and trusted project/shared knowledge
have a mandatory protected floor when topology permits. On topology that cannot meet the floor,
the deployment displays a persistent risk AttentionItem and blocks policy claims that imply the
unachieved guarantee.

Milestone 1 freezes the full-replica small-object cutoff, chunk min/max bounds, transfer-window and
memory maxima, eligible-node and independent-domain rules, offline-target RPO/freshness, repair
deadlines by policy, topology-degradation behavior, and destructive-test fixtures. A critical
policy counts as achieved only while the signed offline/export target is complete and no older
than its configured RPO.

Deduplication is performed only inside the trusted storage service after authorization. APIs,
receipts, quotas, latency classes, and user billing expose logical bytes and never expose chunk
IDs, physical placement, or hit/miss state to a narrower ResourceGrant. Implementations pad or
asynchronously normalize observable ingest behavior where the threat model shows an equality
oracle. A project that requires isolation between resource cohorts uses separate storage domains
and keys rather than project-wide deduplication.

### 10.4 Repair, deletion, and keys

Repair is a system-priority Run with a signed placement plan and fence token. It verifies source
shards, reconstructs into a temporary output namespace, validates the end-to-end content hash,
atomically publishes new placement, and only then retires obsolete shards.

Deletion is manifest-driven, authorization-checked, retention-aware, and receipt-producing.
Shared chunks are removed only when no live project-scoped manifest references them. Project key
rotation and cryptographic erasure require explicit runbooks and recovery proof.

## 11. Shadow master and control-plane failover

### 11.1 Consensus decision

Redline replication slots are not treated as high availability because Redline does not provide
WAL streaming. Redline remains an embedded projection store for each service.

A new jain-control-consensus component uses openraft 0.9.24 behind an internal Jain façade.
OpenRaft's public API is explicitly unstable; the façade isolates the rest of the product from
that churn while using its storage, network, and snapshot replication traits. See
[OpenRaft 0.9.24 documentation](https://docs.rs/crate/openraft/0.9.24).

The pin may change only through a compatibility ADR, complete consensus fault tests, and a
reviewed migration. Application contracts never expose OpenRaft types.

### 11.2 Replicated command journal

All control mutations receive leader-assigned IDs and timestamps and enter an idempotent Raft
command journal. Commands cover:

- projects, membership, grants, policies, and guest leases;
- Run intents, state transitions, attempts, reservations, and gates;
- Git compare-and-swap ref updates, PR/check/release events;
- storage manifests, placement commitments, and repair publication;
- knowledge promotion and campaign state;
- audit events, break-glass grants, node enrollment, fencing, and revocation.

Security time uses a committed hybrid logical clock and quorum-monotonic time floor, not a
leader-local timestamp alone. A new leader cannot assign time earlier than the last committed
floor. The compatibility contract freezes maximum controller skew, floor-update cadence, ticket
and capability leeway, and worker deadline behavior. Controllers outside the skew bound cannot
lead; under unresolved clock uncertainty the system refuses new or renewed guest leases,
break-glass grants, join tickets, decision capabilities, and cursors rather than extending
authority.

Workers translate signed controller expiry into a bounded local monotonic deadline and may expire
earlier, never later, after disconnect or fence. Leadership change, restart, snapshot restore, and
wall-clock rollback tests prove an expired authority cannot resurrect.

Bulk bytes remain in distributed CAS. Raft carries commands, hashes, manifests, small metadata,
and durable locations.

Every artifact-backed mutation uses one publication protocol: stage bytes under an uncommitted
upload/attempt identity, verify hashes and achieved durability, propose the manifest/ref command,
commit it through Raft, then make the manifest reachable. Uncommitted staging is invisible and
garbage-collected by a bounded receipt-producing sweeper. A committed command may never be the
first durable reference to bytes that have not already met its required storage policy.
Large manifests are hierarchical CAS objects; Raft stores only a bounded root descriptor, root
digest, policy, size/count bounds, and durable placement commitment.

Followers apply committed commands into independent Redline and Jeryu projections, then verify
state digests and report lag. At defined committed sequences, every follower computes canonical
projection digests and compares them with the committed reference digest. A mismatch creates an
AttentionItem, removes that follower from read and promotion eligibility, and requires a verified
rebuild before reinstatement. Projection databases are rebuildable. A mutation is acknowledged only after
the Raft command is quorum-committed, required bulk inputs are durably placed, the leader projection
has applied it, and the immutable receipt is readable.

Consensus owns ordering, durable agreement, and replay; it does not take over domain semantics:

| Domain | Semantic validator and command proposer | Committed projection |
|---|---|---|
| Accounts and credentials | Jeryu identity | Jeryu account projection |
| Projects, grants, sessions, knowledge policy, and campaigns | Jain | Jain/Redline projections |
| Git objects, refs, PRs, checks, and releases | Jeryu Git | Jeryu repository projections and distributed CAS |
| Admission, assignment, leases, reservations, and preemption | SmartCluster | SmartCluster scheduler projection and worker dispatch |
| Reasoning DAGs and memory proposals | ZYAL plus host-owned Jain/Jeryu gates | Project knowledge ledger |
| Artifact placement and repair | Jain storage policy plus SmartCluster placement | Artifact manifests and worker shard state |

Every authoritative projection has one writer: its committed-command apply loop. API handlers,
schedulers, Git endpoints, and background tasks validate and propose commands but never mutate a
projection database or ref directly. The existing durable SmartCluster scheduler store becomes a
replayable projection. Capacity observations and heartbeats may remain bounded ephemeral inputs,
but assignment, lease, fence, reservation, preemption, checkpoint, and restart-adoption decisions
are committed before they become externally effective. Single-controller installations use the
same journal and apply path in one-member mode.

Identity commands include account lifecycle, password-verifier updates, MFA enrollment and
revocation, PAT digest and scope changes, SSH public keys, session revocation, lockout state, and
recovery policy. Plaintext passwords, PATs, private keys, and MFA seeds never enter the journal.
Verifier material and any required recoverable secret are envelope-encrypted under deployment
key policy, are redacted from receipts, and are available to an eligible promoted controller only
after its authorized unseal path succeeds.

For Git receive:

1. objects are uploaded into per-receive quarantine and content-verified in CAS using the
   repository's declared Git object format;
2. Jeryu validates reachability, authorization, hooks, branch protection, required checks, object
   closure, and every expected old/new OID as one atomic multi-ref transaction;
3. the leader journals one command binding the full ref set, validation digest, actor, repository
   generation, and quarantined object manifest;
4. the single committed-command apply loop publishes objects and refs exactly once;
5. indexes and code-intelligence projections rebuild from the committed generation;
6. the client receives success only after the acknowledgement rule is met.

No controller process writes a local authoritative ref outside the apply loop. Garbage collection
respects committed refs, open PRs, retained receipts, snapshots, and in-flight quarantine. A failed
or uncommitted receive retires quarantine by policy without making objects or refs reachable.

### 11.3 Read and promotion rules

Followers may serve explicitly stale read-only views. Every response identifies the applied global
sequence and staleness. Security-sensitive reads and all writes go to the leader.

Automatic promotion requires three or five controller-eligible nodes and quorum:

- one node: supported, manual recovery only;
- two nodes: no automatic promotion without quorum;
- three nodes: tolerate one unavailable member;
- five nodes: tolerate two unavailable members.

Promotion fences the prior leader, validates command and storage reachability, restores or verifies
projections, and only then accepts writes. A minority partition never serves mutations.

Acceptance targets:

- zero loss of acknowledged control mutations;
- one authoritative state transition per effect ID or idempotency key under replay;
- no split brain;
- leader recovery within 30 seconds on reference topology.

Command and notification delivery is at least once. Deterministic apply and a transactional outbox
bind every external effect to its committed command/effect ID. Receivers deduplicate when the
protocol supports it; otherwise duplicate notifications are possible and explicitly modeled.
Duplicate or stale attempts may execute after a partition, but fencing prevents their output,
lease, ref, route, or manifest from becoming authoritative.

The consensus qualification suite publishes a mutation-boundary matrix for every command family:
accounts/credentials, projects/grants, guests/break-glass, Runs/leases/reservations, Git/PR/checks,
storage publication/repair/deletion, knowledge/campaigns, services/routes, controller membership,
and audit. Each matrix enumerates failure before staging, after staging, before proposal, after
commit, before/after projection apply, before/after outbox delivery, during acknowledgement, and
during replay, with the idempotency/effect ID, externally observable state, expected receipt, and
cleanup rule at every point.

## 12. Public interfaces and contracts

### 12.1 Unified Jain API

New APIs are under /api/v2:

~~~text
/projects
/projects/:id/members
/projects/:id/resources
/projects/:id/sessions
/sessions/:id/messages
/projects/:id/runs
/runs/:id/pause
/runs/:id/resume
/runs/:id/checkpoint
/runs/:id/cancel
/runs/:id/retry
/projects/:id/attention
/projects/:id/knowledge
/projects/:id/campaigns
/projects/:id/services
/workers
/storage
/admin/users
/admin/guests
/admin/break-glass
/admin/controllers
/events
~~~

REST uses JSON. High-volume live events use Postcard binary frames. Diagnostics and replay exports
use JSON.

Mutations require authenticated actor context, authorization, an idempotency key, and an expected
resource version where lost updates are possible. Responses include the resulting version,
ActionReceipt ID, and global sequence. Validation and authorization failures use stable typed
error codes and do not leak resource existence.

Collection routes support GET and POST; addressable resources support GET and the narrow PUT,
PATCH, or DELETE operations defined by their Rust contract; action suffixes such as pause and
retry are POST commands. The contract freeze publishes exact methods, request/response and error
schemas, authorization scopes, status mapping, rate limits, and audit behavior for every route.

Lists use stable cursor pagination with explicit sort keys and snapshot/consistency semantics,
never an unspecified database order. Strong leader reads are default for mutation-sensitive
state; explicitly requested stale reads identify their applied sequence. ETag/If-Match or the
equivalent typed expected version protects concurrent updates. Idempotency retention and replay
responses are versioned contract values.

Upload and download are streamed, content-length bounded when known, hash-verified, resumable, and
range-capable where the artifact policy permits. /events uses a versioned WebSocket subprotocol
for Postcard frames and a bounded authenticated JSON replay/export endpoint; negotiation, maximum
frame size, replay window, cursor expiry, backpressure, and slow-consumer behavior are frozen.

### 12.2 Rust-owned domain types

The initial contract freeze includes:

- Project, ProjectMembership, GuestProfile, GuestLease;
- Resource, ResourceGrant, WorkspaceBinding;
- RunSpec, RunRecord, WorkloadTemplateId, ExecutionBudget;
- AttentionItem, Gate, ActionReceipt;
- KnowledgeNode, KnowledgeEdge, MemoryCapsule, Campaign;
- StoragePolicy, ArtifactManifest, PlacementReceipt;
- WorkerCapabilities, ProductionReservation, ProductionService, ProductionRevision;
- AuditEvent, BreakGlassGrant.

Rust sources own wire names, optionality, numeric bounds, enum discriminants, version behavior,
and validation. TypeScript contracts are generated and drift-checked. Generated files are never
edited manually.

The freeze does not mean every field is immutable forever. It means changes require explicit
compatibility classification, golden vectors, generated-client drift proof, migration behavior,
and a versioned deprecation window.

The frozen artifact set includes exact Rust definitions, OpenAPI and JSON Schema, assigned binary
discriminants, golden JSON and Postcard vectors, generated TypeScript hashes, error catalogs,
authorization scopes, and a complete preserved /api/v1 read-endpoint inventory.

### 12.3 Live event envelope

Every event carries:

- envelope version;
- deployment ID;
- control-journal global sequence or an authorization-filtered opaque commitment to it;
- event ID and causation/correlation IDs;
- actor;
- project ID, or a reserved deployment scope for deployment-wide events;
- optional session and Run IDs;
- leader-assigned timestamp;
- kind and payload version;
- resume token;
- integrity binding.

Control events use the committed application-log sequence. High-volume chat tokens, log chunks,
metrics, and progress frames do not each become Raft commands; they use a durable per-session or
per-Run stream offset plus the last applicable committed control sequence. Project clients receive
a contiguous stream sequence and opaque resume cursor so gaps do not reveal other projects'
activity. Authorized deployment admins and diagnostics may receive the numeric global sequence.

Resume uses a durable authorized cursor, not an in-memory connection offset. Consumers deduplicate
by event ID, reject regressive sequence within their stream, and recover allowed gaps through
bounded replay. Authorization is re-evaluated during replay. Resume tokens are audience-bound,
expiring, tamper-evident, and invalidated by policy-sequence changes.

### 12.4 Compatibility

- Existing Postcard V1 meanings remain unchanged.
- /api/v1 read compatibility remains for one major release.
- All new mutations use /api/v2.
- Unknown optional JSON fields are tolerated where documented.
- Unknown required binary discriminants, signatures, template versions, or policy versions fail
  closed.
- Internal service adapters negotiate a bounded version window and expose incompatibility as an
  AttentionItem before upgrade.

“One major release” means legacy reads remain supported throughout target major N and N+1 and are
removed no earlier than N+2, after usage telemetry and migration evidence satisfy the published
removal gate. Each Postcard version has an explicit decoder and golden corpus; appending an enum
variant does not by itself authorize binary struct evolution. Milestone 1 freezes the N/N+1
controller, worker, API, event, migration, and rollback compatibility matrix.

During a rolling controller upgrade, consensus maintains an active command-feature set equal to
the intersection supported by every voter that may apply the log. The leader MUST NOT commit a
new command kind, field requirement, or discriminant until every active voter can decode and apply
it and the feature-set transition is itself committed. Unknown required variants fail closed
without partially applying state.

## 13. Security and audit model

### 13.1 Threat boundary

Break-glass constrains deployment-admin actions through the Jain application and its normal
operator APIs. It does not claim confidentiality from operating-system root on a controller or
from a Managed worker that must process authorized plaintext. The default server-held project-key
model trusts approved controller hosts and the selected worker trust class while still enforcing
project and capability isolation between ordinary users, admins, Runs, and workers.

A project that requires protection from host operators MUST use a separately qualified profile
with external or client-controlled key custody, attested confidential-compute workers, an
attestation-bound unwrap path, and an explicit recovery tradeoff. Jain continuously displays the
effective trust class; it never labels ordinary Managed placement as host-admin confidential.

Workers receive only attempt- and chunk-scoped unwrap material or plaintext capability, never a
worker-wide project key. Key material is memory-bounded, non-swappable where supported, zeroized,
excluded from checkpoints and crash dumps, and unusable after the attempt fence or policy sequence
changes.

### 13.2 Capability model

Worker inputs are explicit capabilities bound to project, Run, attempt, fence token, path,
actions, expiry, and content hash where relevant. Capabilities are least-privilege, non-transitive,
and invalid after attempt fencing.

Mounted namespaces are limited to:

- read-only authorized inputs;
- the standalone workspace when the template permits it;
- bounded scratch;
- declared outputs;
- approved devices and network routes.

The host filesystem, controller sockets, credential stores, unrelated projects, primary
checkouts, and arbitrary device nodes are never mounted.

### 13.3 Receipts and denial records

Every quorum-accepted sensitive state action emits an immutable, hash-bound global ActionReceipt
containing:

- actor and effective authorization;
- action and target;
- policy and contract versions;
- request/idempotency identity;
- previous and resulting state hashes where applicable;
- time, global sequence, and controller identity;
- Run/attempt/fence context;
- approval or confirmation evidence;
- terminal result and reason.

Secrets and private payloads are represented by opaque identifiers or keyed, domain-separated
commitments, never copied into receipts. Plain hashes are not used for low-entropy private values
that permit dictionary recovery. Receipts use a versioned closed schema, controller signature,
previous/root hash, and effect ID. Receipt batches are Merkle-rooted into the committed journal,
retained by policy, and independently verifiable from an export. Query and export enforce the same
content authorization as the underlying action.

An authorization, validation, policy, or no-quorum refusal processed while the global journal is
available receives a committed denial ActionReceipt. When quorum is unavailable, no mutation is
accepted and no global ActionReceipt is invented. A controller that authenticates and processes
the refusal writes a signed local DenialRecord in its append-only denial chain and returns its
digest to the caller; denial records are reconciled into the audit journal after quorum recovery.
If even local durable recording is unavailable, the request fails closed with an explicitly
unpersisted, self-verifiable error response and raises an operational AttentionItem when durable
reporting becomes possible. An unprocessed network request has no receipt guarantee.

### 13.4 Required fail-closed controls

The platform MUST refuse, with receipts:

- Git worktree creation or registration;
- direct push by an agent;
- hook or required-check bypass;
- direct primary-checkout mutation;
- symlink escape and path traversal;
- broad destructive host commands;
- secret exfiltration;
- unauthorized data or device mount;
- stale fence-token publication;
- guest privilege escalation;
- unapproved network access;
- unsigned or unapproved workload/toolchain images;
- private-content retrieval before authorization;
- agent attempts to modify host-owned gates.

## 14. Migration and cutover

### 14.1 Existing Jain

Existing Jain sessions import Git objects, checkpoint refs, LFS objects, collaborators, and
artifacts into hidden Jeryu project repositories. Every imported object and ref is content-hash
verified. Session/chat ordering and actor attribution are preserved or explicitly marked
unresolved.

Existing token owners become unclaimed migration principals. Bootstrap admin claims the local
owner. Additional principals require an admin invitation and explicit claim; heuristic email or
token matching cannot silently transfer ownership.

### 14.2 Existing Jekko/ZYAL

Reasoning and memory import into a selected project. Source hashes, timestamps, edges, and
provenance are preserved. Legacy Global memory becomes shared_library_pending and requires
administrator review. Removed worktree or primary-repository modes are imported only as historical
facts; they cannot be resumed.

### 14.3 Existing Jeryu

Repositories, accounts, grants, PRs, checks, releases, objects, and refs preserve Jeryu semantic
authority and identity. Cutover content-verifies them, seeds the committed journal and new
projections at one exact Jeryu generation, and then makes the committed apply loop the sole writer.
The prior stores become read-only rollback and parity inputs; they are never a concurrent
authority. Migration builds Jain project bindings without recreating a competing Git engine.

### 14.4 Cutover method

There is no dual-write period. The required flow is:

1. compatibility preflight and capacity check;
2. signed backup and restore rehearsal;
3. read-only import into the target fabric;
4. content-hash and authorization verification;
5. shadow-read comparison against old surfaces;
6. bounded maintenance window;
7. final delta capture and journal cut;
8. v2 activation and smoke journeys;
9. retained rollback snapshots and explicit rollback decision window.

A signed MigrationParityManifest compares source and target principals, membership and grants,
messages and ordering, Git objects/refs/ACLs/PRs/checks, artifacts and LFS, knowledge/memory edges,
models, and audit roots. Identity, authorization, content hashes, and authoritative refs have zero
tolerance for mismatch. Any explicitly allowed display/cache difference is enumerated, justified,
and excluded from authority. The cutover plan freezes the maximum maintenance duration, final-delta
limit, shadow-read sample and mismatch thresholds, rollback decision duration, and abort gates.

Before v2 accepts mutations, rollback may restore the full compatible control snapshot, projection
state, Git-ref boundary, and artifact manifests. After the first acknowledged v2 mutation, a
pre-cutover snapshot MUST NOT be reactivated by itself. Rollback then requires either a tested
reverse-journal translator that applies every acknowledged compatible mutation to the old
authority before reopening writes, or a read-only incident barrier followed by repair and
roll-forward. If neither path proves zero acknowledged-write loss, rollback is refused. Partial
rollback that would lose writes or reintroduce dual authority is prohibited.

Legacy Jain/Jeryu/Jekko end-user surfaces are removed only after migrated parity, acceptance
journeys, operator signoff, and rollback evidence pass.

## 15. Packaging, backup, restore, and upgrade

The signed release manifest binds:

- release and schema versions;
- all controller and worker binary digests;
- UI asset digest;
- OCI/toolchain image digests;
- internal API and contract versions;
- migration binaries and ordered migration graph;
- default policy digest;
- SBOM, provenance, and signing identity;
- minimum rollback-compatible version.

Offline media contains everything required for installation and approved workloads in the
rehearsal profile. Installation never resolves an unpinned public dependency.

Backup captures a consensus-consistent sequence S, control snapshot, artifact-manifest root, keys
under the deployment key-wrapping policy, Git refs, projection rebuild metadata, and audit roots.
The backup is complete only after a reachability walk proves every chunk/shard referenced at S is
included in the backup or in an explicitly bound durable external target with the required keys
and policy. Restore verifies signatures and hashes and performs a full isolated-media reachability
check before activation.

Upgrade is staged: preflight, snapshot, follower/worker rollout, compatibility validation, leader
transfer where applicable, controller rollout, migration activation, health gates, and receipt.
Rollback is allowed only while schema and command compatibility permit it and is itself
receipt-bound.

## 16. Milestones and exit gates

Milestones are sequential at the program level. Teams may prototype later work in isolation, but
no later milestone can claim exit by waiving an earlier authority or safety gate.

### 16.1 Program foundation

Deliver:

- a recorded UPGRADE_CHAT claim for each documentation or implementation lane;
- this canonical specification and explicit authority ADRs;
- frozen initial domain contracts and golden vectors;
- repository ownership and adapter map;
- baseline UI, scheduler, memory, storage, and control-plane performance;
- an explicit future-major branch and release strategy isolated from 8.0.1;
- a dedicated future-program evidence namespace that is never placed under
  docs/release-evidence/8.0.1/;
- a reviewed reconciliation of the existing Rust-only governing rule with the target's narrow
  Vite/TypeScript/React UI and generated-contract surface before any such source is implemented.

Exit when the program has reviewed authority decisions, contract drift gates, measurable baselines,
and machine proof that no 8.0.1 candidate metadata or release evidence changed.

### 16.2 Identity and project foundation

Deliver Jeryu authentication behind the Jain gateway, projects, memberships, grants, guest-profile
contracts, audit events, and the unified BFF.

Exit when a user can create and share a project and the same authorization result is proven across
chat, data, Git, knowledge, and Run APIs.

### 16.3 Unified shell and Runner Board

Deliver persistent chat, Overview/Code/Knowledge, keyboard navigation, project drill-down,
deployment-wide admin Runner Board, read-only Jeryu/SmartCluster aggregation, and Jain-owned
action adapters for the actions required by the milestone exit.

Exit when all current runner states and blockers are represented and actionable from one canvas,
with accessibility and performance gates passing.

### 16.4 Universal workload fabric

Deliver production priority and reservations, WorkloadTemplateId, node join, SSH bootstrap,
capability-bound artifacts, and adapters for Jeryu CI, agent sessions, and Jain workloads.

Exit when inventory and runtime proofs show no Jain or Jeryu execution workload covered by the
Run definition bypasses SmartCluster.

### 16.5 Governed Git and live coding

Migrate session micro-Git to Jeryu, replace companion shells with governed execution, remove all
ZYAL worktree modes, and enforce host-mediated commits, refs, PRs, and Jankurai gates.

Exit with concurrent multi-session branch, compare-and-swap conflict, PR, cleanup, and
prohibited-worktree proofs.

### 16.6 ZYAL knowledge and evolution

Deliver headless ZYAL networks, reasoning DAGs, scoped promoted memory, search/evidence, Knowledge,
Campaigns, and archive/reopen behavior.

Exit when dataset-only InnovationZero and repository-backed IP campaigns use the same Run,
knowledge, gate, and receipt model.

### 16.7 Distributed storage

Deliver streaming, compression, project-scoped deduplication, encryption, erasure coding,
placement, repair, risk policies, Git/LFS integration, and migration tooling.

Exit after destructive node-loss, bit-rot, topology-degradation, key-rotation, and repair testing
at every advertised tolerance.

### 16.8 Shadow-master HA

Deliver the Raft command journal, follower projections, Git-ref replay, snapshots, stale read-only
followers, promotion, fencing, and recovery tooling.

Exit after partition, crash-between-commit-and-apply, stale follower, snapshot restore, and repeated
leader-failure tests prove no split brain or acknowledged mutation loss.

### 16.9 Production packaging and cutover

Deliver the signed controller/worker bundle, offline installer, upgrade/rollback receipts,
AtomicSoul canary, on-premises air-gap rehearsal, migration tools, and operator runbooks.

Exit only after identical-digest deployment proof, migration parity, rollback rehearsal, security
acceptance, and final mixed-workload soak.

## 17. Required end-to-end acceptance

Each journey records actor, policy versions, Run IDs, exact artifact and Git hashes, global
sequences, receipts, and final authorization/storage state.

1. Upload a dataset, automatically create a project, run InnovationZero, inspect evidence, promote
   a model, and archive the Campaign.
2. Import a full repository, run three concurrent agent sessions on separate branches, pass
   Jankurai, open protected PRs, and resolve a branch conflict.
3. Share a joint project with another authenticated user and prove shared code, data, and compute
   with private-resource isolation.
4. Admit floating guests, exhaust the pool, log in as an authenticated user, and verify guest work
   checkpoints and requeues according to policy.
5. View every workload type on the project Runner Board and drill from Run to worker, branch,
   artifact, knowledge, and blocker.
6. Add a worker through join-command and SSH-bootstrap paths, rotate its identity, fence it, and
   revoke it.
7. Lose the maximum advertised number of storage nodes, rebuild shards, and verify every retained
   content hash.
8. Kill the controller leader at each mutation boundary and prove each acknowledged effect has one
   authoritative state transition after at-least-once replay.
9. Attempt worktree creation, direct push, hook bypass, symlink escape, path traversal, secret
   exfiltration, unauthorized data mount, guest escalation, stale-attempt publication, and
   unapproved network access; all fail closed with receipts.
10. Exercise audited break-glass access and verify reason capture, owner notification, expiry,
    post-expiry denial, and non-transitive permissions.
11. Install the exact same signed bundle and digests in connected AtomicSoul and fully air-gapped
    on-premises environments.
12. Import representative Jain sessions, checkpoint refs, LFS, collaborators, artifacts, and
    unclaimed principals; prove exact migration parity and explicit claim behavior.
13. Import Jekko reasoning, negative memory, and legacy Global memory; prove provenance,
    shared_library_pending isolation, and refusal to resume removed workspace modes.
14. Bind existing Jeryu accounts and repositories at an exact cutover generation; prove
    objects/refs/ACLs/PRs/checks, read-only old-store behavior, and single-writer projection state.
15. Prove /api/v1 read parity, backup and isolated-media restore, compatible rolling upgrade,
    pre-write rollback, and preservation or refusal of every acknowledged post-v2 mutation.
16. Seed canary secrets and scan logs, databases, journal entries, receipts, telemetry, process
    titles, environment, and shell history; then attempt receipt deletion, rewrite, truncation,
    and fork and verify audit-root detection.
17. Roll a ProductionService to a new signed Revision under live traffic, fail health gates,
    reclaim borrowed capacity, lose a serving worker and then the controller leader, and prove
    SmartCluster replacement, route safety, rollback, and reservation behavior.

## 18. Performance and reliability gates

On versioned reference hardware and data fixtures:

| Gate | Target |
|---|---|
| Warm view switch | p95 below 75 ms |
| Runner event receipt-to-paint | p95 below 100 ms |
| Chat token gateway-to-paint | p95 below 150 ms |
| Initial local-LAN interactive shell | below 1.5 seconds |
| Single-node controller idle RSS | below 1 GiB, excluding OS page cache |
| Worker idle RSS | below 200 MiB |
| Browser heap with 10,000 visible Run records | below 250 MiB |
| Knowledge graph | 100,000 stored nodes via aggregation; at most 2,000 rendered |
| Leader recovery | below 30 seconds with zero acknowledged mutation loss |

SmartCluster submit, schedule, pressure-response, and restart-adoption SLOs MUST be preserved or
tightened from the accepted baseline. Each gate defines warmup, sample count, percentile method,
hardware, topology, dataset, and failure threshold before milestone qualification.

Milestone 1 publishes a signed performance fixture manifest containing exact hardware/firmware,
OS/kernel, browser, network topology and impairment, controller/worker layout, dataset and event
fixtures, warmup, sample count, percentile algorithm, instrumentation boundaries, accepted
SmartCluster baseline values, and comparison tolerance. It also defines numeric soak thresholds
for queue growth, lease cleanup, workspace cleanup, projection lag, repair deadline, error budget,
and resource growth; qualitative “unbounded” or “leaked” judgments cannot qualify a milestone.

A 72-hour soak mixes CI, interactive coding, ZYAL, ML, production, storage repair, guests,
controller failover, worker loss, and artifact migration. It fails on leaked leases, orphaned
workspaces, unbounded queues, unrepaired retained artifacts, divergent projections, lost receipts,
authorization drift, or durability misreporting.

## 19. Architecture decision records

These decisions are accepted by this specification. Reversal requires a superseding ADR,
compatibility and migration analysis, security review, and owner approval.

| ADR | Decision | Consequence |
|---|---|---|
| SM-001 | Jain is the sole shell, gateway, and brand authority | Jeryu and Jekko user-facing shells are removed after parity |
| SM-002 | Jeryu accounts are deployment identity authority | Jain token-derived identity is migrated to claimable principals |
| SM-003 | Jeryu is the sole Git object/ref and PR/check authority | Jain session Git storage becomes hidden Jeryu repositories |
| SM-004 | SmartCluster is the sole workload scheduler | All other schedulers become projections or are removed |
| SM-005 | ZYAL is headless and scheduled through SmartCluster | Jekko application surfaces and worker pool do not survive |
| SM-006 | Git worktrees are prohibited without exception | Standalone no-local exact-SHA checkouts are the only code workspace |
| SM-007 | Hosts, not agents, mutate refs and open PRs | Agent capabilities exclude push credentials and primary repo writes |
| SM-008 | One project-scoped distributed CAS serves all artifact classes | Jeryu retains ref authority; controller-local bulk stores retire |
| SM-009 | Authorization precedes semantic retrieval | Private content cannot leak through embeddings, counts, or ranking |
| SM-010 | Admin private-content access is one-hour break-glass | Global operations visibility does not imply content visibility |
| SM-011 | Control HA uses a Raft command journal and rebuildable projections | Redline slots are not represented as WAL-based HA |
| SM-012 | OpenRaft is pinned behind an internal façade | Its unstable public API cannot escape into product contracts |
| SM-013 | No dual-write migration | Cutover uses import, shadow reads, maintenance, and rollback snapshots |
| SM-014 | AtomicSoul and on-prem use identical digests | Environment differences are configuration, not product forks |
| SM-015 | Rust owns contracts and generates TypeScript | Hand-edited generated clients and divergent validators are refused |

## 20. Traceability matrix

| Required outcome | Owning proof |
|---|---|
| One Jain experience | Shell navigation, deep-link, persistent-chat, and legacy-surface parity journeys |
| One execution authority | Runtime inventory plus SmartCluster admission/lease receipts for every template |
| No Git worktrees | Static reachability scan, hostile runtime tests, sandbox receipts, and cleanup proof |
| Governed code changes | Exact-base clone, host CAS branch, Jankurai, protected PR, and conflict journeys |
| Scoped knowledge | Authorization-before-ranking and shared-library promotion isolation tests |
| Floating guests | Pool exhaustion, priority admission, checkpoint/requeue, expiry, and escalation tests |
| Distributed durability | Node-loss, bit-rot, repair, achieved-policy, and content-hash proofs |
| Private admin access | Break-glass reason, notification, expiry, denial, and audit-root proof |
| Control failover | Boundary fault injection, replay, fencing, quorum, and no-loss proof |
| Deployment parity | Signed manifest and exact digest comparison across connected and air-gapped installs |
| 8.0.1 independence | Before/after hashes and path-state checks for candidate authority and evidence |

## 21. Implementation constraints and defaults

- Jain is the sole product shell and brand authority.
- Jekko contributes engines only; its TUI, chat, server-session, and web applications do not
  survive.
- Jeryu remains Git and account authority but is not separately exposed as a product.
- SmartCluster is the sole scheduler and lease authority.
- Platform services and tooling use Rust; the target product UI uses Vite, TypeScript, and React
  only after the governing Rust-only policy is explicitly amended through its reviewed authority.
- No new Python control-plane code is introduced.
- Imported projects may be polyglot only through signed administrator-allowlisted toolchain images.
- Balanced storage is the default; critical control data has a protected floor when topology
  permits.
- Production reservations cannot be preempted by ordinary R&D.
- Automatic shadow-master failover requires at least three controller-eligible nodes.
- Admin access to private content is break-glass only.
- No Git worktree is created by a user, agent, service, CI job, migration tool, test, or cleanup
  procedure.
- Current 8.0.1 candidate metadata, branch protection, immutable tags, reviewed lifecycle,
  fail-closed GA posture, and rollback authority remain untouched.

## 22. Deferred choices

The following choices may be resolved during the named milestone without weakening an accepted
decision:

- the exact UI component library and graph renderer;
- the internal façade shape around OpenRaft;
- chunker polynomial and min/max chunk bounds around the 4 MiB average;
- the exact controller snapshot cadence;
- supported sandbox backends per operating system;
- the first approved polyglot toolchain image set;
- archive TTL defaults for recomputable scratch data.

Each choice requires measurable alternatives, a selected owner, compatibility impact, threat
analysis where relevant, and an ADR or contract amendment. None may introduce another product
shell, scheduler, identity authority, Git authority, bulk store, worktree path, or deployment fork.

---

This specification defines the future-major target. It is not permission to mutate the current
release candidate. Implementation proceeds only through separately claimed, repository-owned,
reviewed work that remains isolated from and does not delay 8.0.1.
