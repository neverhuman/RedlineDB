# Phase 6 Candidate 1 — Morsel/Vector Execution Model

Status: design draft (Phase 6, post v5.0.0 tag). No Rust source changes proposed here; this document fixes the shape that the M1–M8 sub-WS will implement.

Source plan: `/home/ubuntu/.claude/plans/please-make-sure-you-typed-stallman.md` lines 165–175 and 429–438.
Sibling perf spec: `tips/performance/helper/redlinedb_theoretical_limit_engineering_spec.md` waves 4 and 6.

## 1. Executive summary

Phase 5 closed the median-latency gap to SQLite by fixing the planner (WS-A1/A2/A7), the compile chain (WS-B1/B2), and dot-command bulk paths (WS-C5). The residual gaps are structural: `crates/sql/src/exec/agg/group.rs::execute_grouped_select` still materialises `Vec<Vec<SqlRow>>` per group (3–10× cliff on `GROUP_BY_HAVING`, `AGGREGATE_FUNCTIONS_CORE`, `AGG_GROUP_HAVING_059`), filter/project still walk `SqlValue` one row at a time, and every cross-node hop heap-allocates a row vector. The Morsel/Vector model batches 256–1024 rows into columnar `Morsel`s with a validity `Bitmap`, runs SIMD-friendly kernels per column, and only materialises tuples at the render boundary. Expected wins on named parity cases: `GROUP_BY_HAVING` flips to ≤ 0.4× (currently 9.34×), `AGGREGATE_FUNCTIONS_CORE` flips to ≤ 0.4× (currently 3.09×), large covering scans pick up 2–8×, and per-batch memory drops 30–80% because text/blob payloads stay in an arena instead of one `Vec<u8>` per cell. Cost is ~5,000 LOC across eight sub-WS, a router that gates the new executor by query shape, and a permanent boundary copy back to `SqlRow` for surfaces (FFI, RETURNING, triggers) that still consume tuples. It comes after Phase 5 because the kernels reuse `SnapshotView<'a>` (`crates/kernel/src/index/cursor.rs:87`) and `RawIndexCursor` (`crates/kernel/src/index/cursor/raw/range.rs`), and after Phase 6 Candidate 5 (AccessPath IR) because the router decides per-AccessPath whether to dispatch the vector path.

## 2. Type design

All types live under a new module tree rooted at `crates/sql/src/exec/morsel/`. `SmallVec` widths are chosen so a steady-state 1024-row morsel of 8 columns stays inline (no heap), and tiny result sets (≤ 16 rows) never spill any inline buffer.

```rust
// crates/sql/src/exec/morsel/mod.rs
use smallvec::SmallVec;

/// One unit of work flowing between vectorised operators.
/// `len` is bounded by `MAX_BATCH_ROWS` (currently 1024).
pub struct Morsel {
    pub columns: SmallVec<[ColumnBatch; 8]>,
    pub validity: Bitmap,
    pub len: u16,
}

/// Per-column storage. Variants stay narrow: the planner is responsible
/// for picking the right ColumnBatch shape at bind time, not at run time.
pub enum ColumnBatch {
    I64(SmallVec<[i64; 256]>),
    F64(SmallVec<[f64; 256]>),
    Text(BytesArena),
    Blob(BytesArena),
    /// All-NULL column. Carries no payload; validity bitmap is all zero.
    Null(()),
}

/// 1024-bit inline bitmap, spills to heap past that.
/// 16 × u64 = 1024 bits = `MAX_BATCH_ROWS`.
pub struct Bitmap(SmallVec<[u64; 16]>);

/// Borrow-friendly variable-length payload store.
/// One BytesArena per Text/Blob column per morsel; entries live until
/// the morsel is dropped or flushed at the render boundary.
pub struct BytesArena {
    buf: SmallVec<[u8; 4096]>,
    offsets: SmallVec<[u32; 256]>, // offsets[i]..offsets[i+1] is row i's slice
}
```

Invariants (debug-asserted in builders, no run-time check on the hot path):

- `columns[i].len() == len` for every non-`Null` variant.
- `validity.bit_len() >= len`.
- For `Text`/`Blob`: `offsets.len() == len + 1`, `offsets` is monotone non-decreasing, `offsets[len] == buf.len()`.
- `len <= 1024`.

The validity bitmap is shared across the whole morsel (one row is "valid" or it has been filtered out); per-column NULL is encoded by `ColumnBatch::Null(())` for all-NULL columns and by a dedicated per-column `nulls: Bitmap` carried alongside `ColumnBatch` for partially-NULL columns. The exact split between morsel-level validity and per-column nulls is owned by `morsel/column.rs` — see §3 M1.

## 3. Decomposition into sub-WS

| Sub-WS | Scope | Files (new unless noted) | LOC | Hours |
|---|---|---|---:|---:|
| **M1** | Morsel type + builder + bitmap + arena. Debug invariants, smallvec sizing benches, `MorselBuilder::push_row` for the tuple-to-vector adapter, `MorselBuilder::finish()`. | `crates/sql/src/exec/morsel/{mod,column,bitmap,builder}.rs` | ~800 | 16 |
| **M2** | Morsel scan source. Wrap `RawIndexCursor` (`crates/kernel/src/index/cursor/raw/range.rs`) and the heap-scan path so each `next_batch()` fills one morsel. Reuses `SnapshotView<'a>` for lock-free reads. Reuses `IndexScanScratch` from Phase 5 WS-A4. | `crates/sql/src/exec/morsel/scan.rs`, plus a thin adapter in `crates/sql/src/exec/index_access.rs` (modify, no behaviour change to tuple callers) | ~700 | 18 |
| **M3** | Vectorised filter SIMD kernels. `cmp_i64_lt_const`, `cmp_i64_eq_col`, `cmp_text_eq_const`, plus the dispatcher. Mirrors the `is_x86_feature_detected!` + `unsafe_ledger.toml` pattern used by `crates/kernel/src/vector/simd.rs`. Falls back to scalar on aarch64 and pre-AVX2 x86. | `crates/sql/src/exec/morsel/filter.rs`, `.jankurai/unsafe-ledger.toml` (append entries) | ~700 | 24 |
| **M4** | Vectorised projection + arithmetic. `add_i64_i64`, `mul_f64_f64`, `coalesce`, `substr_const_const`, plus an emit kernel that writes from input column to output morsel respecting the active validity bitmap. Designed to be Tier-0 lower for the future ScalarVM (Candidate 4) — see §5. | `crates/sql/src/exec/morsel/project.rs`, `crates/sql/src/exec/morsel/arith.rs` | ~600 | 20 |
| **M5** | Vectorised hash-agg replacement. Rewrites `crates/sql/src/exec/vec/hash_agg.rs` to accept `Morsel` directly instead of `SqlRow`, with SoA aggregate state (`sum: Vec<i64>`, `count: Vec<u64>`, …) and inline `[u8; 24]` small-key probing per the wave-6 spec. Routes from `crates/sql/src/exec/agg/group.rs::execute_grouped_select` when M7 says yes. Reuses `AccState::merge` for parallel finalisation (Phase 5 WS-C2). | `crates/sql/src/exec/vec/hash_agg.rs` (rewrite), `crates/sql/src/exec/agg/group.rs` (router only) | ~900 | 32 |
| **M6** | Adaptive batch width policy. Picks batch width per query: 1024 for cardinality > 10⁴, 256 for 10²–10⁴, 64 for < 10², bypass entirely (tuple path) for < 16. Telemetry via `Phase11Counters::morsel_width_chosen`. | `crates/sql/src/exec/morsel/width.rs`, extend `crates/sql/src/exec/telemetry.rs` | ~400 | 12 |
| **M7** | Executor router. The single gate that decides per-statement whether to dispatch the morsel path or stay on the tuple path. Inputs: AccessPath shape (Candidate 5 IR), projection shape, presence of UDFs / triggers / RETURNING. Pragma opt-out: `PRAGMA morsel_executor = OFF`. | `crates/sql/src/exec/morsel/router.rs`, `crates/sql/src/exec/mod.rs` (one call-site) | ~500 | 16 |
| **M8** | Morsel → tuple flush. The boundary copy back to `SqlRow` for surfaces that cannot consume morsels (FFI cursor step, RETURNING into trigger row-context, `Connection::query` iterator). Lives at the executor edge so the tuple path never observes morsel internals. | `crates/sql/src/exec/morsel/flush.rs` | ~400 | 10 |
| **Total** |  |  | **~5,000** | **148** |

## 4. Internal sequencing

```
M1 (type + builder)
 ├─→ M2 (scan source)         needs Morsel + Bitmap shape stable
 ├─→ M3 (filter SIMD)         needs ColumnBatch + Bitmap stable
 └─→ M4 (project/arith)       needs ColumnBatch + Bitmap stable
       │
       ├─→ M5 (hash-agg)      needs M2 (source) + M3 (predicate eval)
       │
       └─→ M8 (tuple flush)   needs Morsel stable; runs in parallel with M5

M6 (adaptive width)            depends on M2 telemetry only
M7 (router)                    depends on M1–M6 all stable; lands last
```

Strict ordering: **M1 first**; **M7 last**. Inside the diamond, M2/M3/M4 can be three parallel PRs once M1 is on `main`. M5 must wait on M2 and M3 (it consumes filtered morsels). M8 can land in parallel with M5 because it touches only the egress side. M6 can land anywhere after M2.

Calendar with two agents: M1 (week 1) → M2 + M3 + M4 (weeks 2–3) → M5 + M6 + M8 (weeks 3–4) → M7 + integration (week 5) → harness + soak (week 6). Single-operator at 6h/day: ~25 working days.

## 5. Cross-candidate dependencies

This candidate **subsumes Phase 5 WS-B8 Part 1** (RowView/arena rows). The arena rows in WS-B8 were a stepping stone toward exactly the `BytesArena` + late-materialisation model spelled out here. If both ship, do **M1 + M2 in place of WS-B8 Part 1** — same engineering budget, strictly more useful payoff.

Other cross-candidate edges:

- **Candidate 5 (AccessPath IR)** must land before M7, because the router keys on AccessPath shape. If AccessPath IR slips, M7 falls back to gating on `IndexAccessMatch` + projection shape — workable but lossier.
- **Candidate 4 (Two-Tier ScalarProgram VM)** overlaps with M4. Joint design: M4 kernels are the Tier-0 evaluator the ScalarVM will lower into; ScalarVM Tier-1 generates per-statement specialisations that call the same kernels. Concretely: keep M4's kernel signatures stable (`fn add_i64_i64(lhs: &[i64], rhs: &[i64], out: &mut [i64], valid: &Bitmap)`) so ScalarVM emits direct calls without a second copy.
- **Candidate 2 (redlinedb-lite)** is independent; M7 must respect the lite build (router compiled out when the morsel module is feature-gated off).
- **Candidate 3 (WAL Group-Commit Pipeline)** is independent; no shared surface.

## 6. Correctness gates (per sub-WS)

Every sub-WS lands with a differential test in `crates/sql/tests/differential_morsel_vs_tuple.rs` (introduced by M1, extended per WS). The harness runs each parity case from `redline-testing` through **both** executors with the same seed and asserts byte-identical result-set comparison via the existing `parity_coverage.rs` row equality helper.

| Sub-WS | Gate |
|---|---|
| M1 | `morsel_invariants_hold_after_builder_finish` (proptest, 10⁵ random shapes). |
| M2 | `morsel_scan_yields_identical_rowids_to_raw_cursor` on every covering and non-covering case in the parity corpus. |
| M3 | `filter_kernel_matches_scalar_eval` proptest over all supported (op, type) combinations; SIMD vs scalar diff is zero. |
| M4 | `project_arith_matches_scalar_eval` proptest; NULL propagation matches `crates/sql/src/exec/expr/eval.rs`. |
| M5 | `vec_hash_agg_matches_serial_group` over the 27 GROUP BY parity cases; assert spill behaviour matches Phase 5 `HashAggregator` exactly. |
| M6 | `adaptive_width_never_regresses_tiny_result_sets`: ≤ 16-row result sets bypass to tuple path; assert via `Phase11Counters::morsel_width_chosen == 0`. |
| M7 | `pragma_morsel_executor_off_disables_path`; `router_picks_tuple_for_udf_aggregates`; `router_picks_tuple_for_returning`. |
| M8 | `flush_round_trip_is_identity` on every supported ColumnBatch shape; FFI cursor step over morsel results yields identical bytes to tuple-path FFI. |

**Hard gate at every wave boundary**: the existing 1729+ tests stay green. The differential harness extends the gate set; it does not replace it.

PRAGMA opt-out is mandatory: `PRAGMA morsel_executor = OFF` forces the tuple path for the connection, used both by the differential harness and by users who hit a result-set divergence in the field.

## 7. Risk register

| Risk | Mitigation |
|---|---|
| SIMD path off on non-x86 — must compile and pass on aarch64 CI. | M3 kernel module exposes a scalar fallback gated by `#[cfg(not(any(target_arch = "x86_64")))]`. The dispatcher (mirroring `crates/kernel/src/vector/simd.rs`) picks scalar at runtime when AVX2 is absent. aarch64 CI runs the differential harness; correctness is identical, perf is the documented portable-release baseline. |
| Result-set divergence on edge cases: NULL semantics, collation, JSON. | Differential harness (§9) is the safety net. The router refuses morsel dispatch for any expression that references a non-BINARY collation (until M4 grows collation-aware kernels) and for any JSON-valued expression (until JSONB bytecode from Phase 5 WS-B7 lands a morsel-aware variant). |
| Memory regression on tiny result sets. | M6 adaptive width: bypass to tuple path below 16 rows; pick 64-row morsels for 16–100 rows so smallvec inline storage covers the whole batch. Telemetry surfaces `morsel_width_chosen=0` for every bypassed case. |
| Borrow lifetime explosion: `BytesArena` references can outlive the morsel. | M8 flush owns the only copy-out path; everywhere else the morsel is consumed by-value via `next_batch(&mut Morsel)`. Lifetimes are statement-frame scoped, mirroring `IndexScanScratch` from WS-A4. |
| Triggers / RETURNING / FFI cursor step need tuples. | M7 forces tuple path for any statement with a fire-hook trigger or RETURNING; FFI cursor step uses M8 flush. The morsel path is purely an internal optimisation. |
| UDF aggregates do not vectorise. | **Open question** — see §10. M7 forces tuple path for any aggregate that is not built-in; this is correct but leaves UDF-heavy workloads on the slow path indefinitely. |

## 8. Sequencing across Phase 6 candidates

Per the master plan (lines 165–175): **#3 (lite) → #1 (AccessPath IR) → #2 (Morsel) → #4 (ScalarVM) → #5 (WAL pipeline)**.

Morsel/Vector slots **third**: after `redlinedb-lite` (Candidate 2, mechanical refactor, derisks the build-system shape morsel will need to respect via M7 feature-gating), after AccessPath IR (Candidate 5, M7 keys on it), and **before** ScalarVM (Candidate 4, depends on M4 kernel signatures being stable) and the WAL pipeline (Candidate 3, no shared surface but scheduled last for crash-safety risk reasons).

If AccessPath IR slips out of the window, M7 degrades gracefully to gating on existing `IndexAccessMatch` shapes; the candidate ships either way.

## 9. Verification harness

The single source of truth is the `redline-testing` parity corpus (`/home/ubuntu/redline-testing/`), invoked exactly as Phase 5 invoked it (`just perf-full`). M1 introduces `crates/sql/tests/differential_morsel_vs_tuple.rs`:

- Loads every parity case from the external corpus.
- For each case, builds a Connection with `PRAGMA morsel_executor = OFF` and another with the default, runs the case on both, asserts byte-identical result sets via the row equality helper from `crates/sql/tests/parity_coverage.rs`.
- Records per-case morsel width (M6 telemetry) and dispatch decision (M7 telemetry) into a JSONL sidecar for external report review, including cases where the morsel path was expected to fire but did not.
- Runs in `just fast`, fast-fails on the first divergence with a `PRAGMA morsel_executor = OFF` reproducer printed to stderr.

Per-wave: after M2/M3/M4 land, the harness is in place but covers only scan/filter/project shapes. After M5 lands, it covers GROUP BY shapes. After M7 lands, it covers the full corpus. The 1727+ existing tests stay green throughout; the harness is additive.

Performance numbers come from the same `phase5-baseline.jsonl` lineage: run `just perf-full BIN=target/release-pgo/redlinedb OUT=phase6-morsel-pgo`, then review the comparison through the verified external `redline-testing` report workflow.

## 10. Open questions surfaced by this design

- **UDF aggregates in the vectorised path.** M7 currently forces tuple-path for any user-defined aggregate. SQLite's `sqlite3_create_function_v2` UDFs are row-at-a-time by construction; the morsel path would need a `sqlite3_morsel_step()` extension to the UDF ABI (passing column batches + validity instead of `sqlite3_value*`), and any UDF that does not opt in stays on the tuple path. This is a non-trivial ABI surface change and is intentionally **not scoped into M5** — it needs a separate candidate (or a Candidate 4 ScalarVM follow-up that handles UDF lowering uniformly with built-ins). Until then, the gap is documented behaviour: UDF-heavy aggregate queries see no morsel speedup.
- **Collation-aware text kernels.** M3 ships BINARY-only; NOCASE and RTRIM collations stay on the tuple path. Whether to grow collation-aware SIMD kernels or to widen M8 flush to handle "morsel for everything except the collated predicate" is unresolved.
- **JSONB morsel variant.** Phase 5 WS-B7 ships JSONB bytecode for the tuple path. A morsel-aware JSONB executor would need to thread `BytesArena` through the JSON path operators. Out of scope for M3–M5; scheduled as a Phase 6 follow-up if the JSON parity cases remain above 1.0× after Morsel/Vector ships.
