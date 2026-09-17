# Paper Production Plan — RedlineDB IEEE Tier-1 Submission

## Goal

An 8–10 page IEEE-conference-format paper that introduces RedlineDB as a Rust-native, concurrent-write, MVCC embedded SQL database that retains the SQLite drop-in API contract while measurably out-scaling SQLite on mixed concurrent workloads. Camera-ready quality: clean LaTeX, EPS figures, real benchmark data, full bibliography, reproducible artifacts.

## Working title (with hook)

> **RedlineDB: A Rust-Native, Concurrent-Write Embedded SQL Engine That Stays SQLite-Compatible Without Inheriting Its Concurrency Cliff**

Alternate hook (one-liner abstract opener):
> *Forty thousand lines of safe Rust replicate SQLite's API while delivering 8× throughput on 64-thread mixed workloads.*

## Page budget (10 pages)

| § | Title | Pages |
|---|---|---|
| Abstract + Index Terms | | 0.25 |
| I | Introduction | 1.25 |
| II | Background & Related Work | 1.0 |
| III | System Architecture | 2.5 |
| IV | Implementation & Engineering | 1.5 |
| V | Methodology | 0.75 |
| VI | Evaluation | 2.25 |
| VII | Discussion | 0.5 |
| VIII | Conclusion | 0.25 |
| Appendix | Architecture diagram + bench commands | 0.5 |
| References | | 0.5 |
| **Total** | | **~10 pp** |

## Paper structure (one-line takeaways per section)

1. **Abstract (≤ 250 words).** Sets stakes: SQLite is everywhere; concurrency model is a known cliff. We rebuilt it in safe Rust at ~38 KLOC vs SQLite's ~250 KLOC, kept the C ABI, and measured 4×–8× throughput at 32–64 threads on mixed workloads, parity at 64-thread point reads, with zero-lost-acked-commits across 24 failpoint scenarios.
2. **I. Introduction.**
   - SQLite ubiquity: 1T+ deployments, browsers, phones, IoT.
   - Strengths: file format stable since 2004; serializable by default; single-writer simplicity.
   - Cliffs: single-writer WAL, coarse type system (5 affinity classes), no native concurrent multi-writer, hard-to-extend.
   - Our contribution: rewrite in Rust, MVCC kernel, concurrent writers with disjoint-row parallelism, drop-in SQLite C ABI, smaller code base.
   - Contributions list: (1) MVCC + B-link-style index lifecycle in safe Rust; (2) a SQLite-compatible C ABI shim path; (3) deterministic crash certification; (4) reproducible bin-packed parallel benchmark harness; (5) measured 8× scaling vs SQLite on mixed OLTP at 64 threads.
3. **II. Background.**
   - SQLite WAL design and the single-writer constraint.
   - MVCC vs locking; OCC vs pessimistic locks.
   - Embedded DBs in Rust (libSQL, sled, redb, surreal-key); positioning RedlineDB.
4. **III. System Architecture.**
   - Crate hierarchy diagram (`paper/figs/architecture.eps`): kernel → sql → redlinedb → ffi → bench.
   - Storage: 16 KiB slotted heap pages, MVCC per-version chains, per-relation row directories, durable rowid B-tree.
   - WAL: `WalCoordinator` group commit, segment rotation, sync counters, recovery replay.
   - Catalog: `WalPayload::CatalogSnapshot`, `save_atomic` with parent fsync, recover-via-WAL.
   - B-tree index: `(logical_key, row_id)` physical-key navigation handles duplicate runs; range scans terminate early on bound; transactional `insert_tx`/`delete_mark_tx`/`undelete_mark_tx`.
   - Concurrency: `RowLockManager` with relation-aware keys, configurable busy timeout, MVCC snapshots via `Csn` allocator, `BeginMode { Deferred, Immediate, Exclusive }`.
   - Query path: `sqlparser` 0.61 SQLite dialect → planner emits `AccessPath::{TableScan, IndexPointLookup, IndexRangeScan, RowIdGet}` (planner-executor consistency invariant gated by `access_path_is_consumable_by_executor`); EXPLAIN tags index name + kind.
   - DML: `execute_insert`/`update`/`delete` drive both heap and physical indexes, with per-tx `IndexUndoOp` log replayed on rollback or commit failure.
   - FFI: `rldb_*` native ABI plus `sqlite3.h` compatibility header (PR-target for full `sqlite3_*` symbol shim).
5. **IV. Implementation & Engineering.**
   - Safety story: only crate-internal `unsafe` is in `crates/ffi`; kernel and SQL layers are 100% safe Rust (modulo one well-audited thread-local).
   - LOC table (RedlineDB ≈ 38 KLOC active source vs SQLite ≈ 250 KLOC).
   - Failpoint discipline: feature-gated `fail_point!` macros at WAL/commit/heap/index/catalog sites; child-process oracle with fsynced ack log; 16 failpoint sites verified for zero lost-acked-commits across 24 scenarios.
   - Bench harness: bin-packing parallel scheduler (Lane BH), real warmup accounting, `--with-strace` Linux passthrough, recursive `data_bytes`, BUSY/LOCKED/timeout split metrics.
   - Reproducibility: `phase9-baseline` … `wave7-fused` git tags, manifest carries git SHA + Docker digest + host fingerprint + PRAGMA snapshot + checksums.
6. **V. Methodology.**
   - xbabe1 host: 128 vCPU x86_64, Linux 6.8, ext4, Docker 29.2.1, Rust 1.95.0, SQLite 3.x bundled by `rusqlite` 0.37.
   - Workload matrix: 8 workloads × 2 durabilities × 8 thread levels (1, 2, 4, 8, 16, 32, 64, 128) × 5 reps + 1 warmup × 2 engines = **1728 child runs** total.
   - Per-run telemetry: throughput, p50/p95/p99/p999/max, BUSY/LOCKED/timeout, RSS, fdatasync/pwrite counts, recursive data_bytes, integrity checksum.
   - Recovery: 36-case matrix across 3 scenarios × 2 durabilities × 6 reps with crash injection.
   - Failpoint: 24 cases × strict durability across 7 sites (WAL write/fsync, commit-publish, heap mut, index mut, catalog rename, checkpoint).
   - Compat: 40 SQL-logic cases, 0 failures.
7. **VI. Evaluation.** (the heart of the paper)
   - **Fig 1**: throughput vs threads, log-y, 5 representative workloads, both engines.
   - **Fig 2**: p99 latency vs threads, both engines.
   - **Fig 3**: ratio bars (Redline / SQLite) at 1, 8, 32, 64, 128 threads per workload.
   - **Fig 4**: scaling efficiency (qps@N / qps@1) vs ideal linear line.
   - **Fig 5**: failpoint-matrix and recovery-matrix outcomes.
   - **Table I**: headline qps + p99 at the four hero (workload, threads) pairs.
   - **Table II**: LOC + crate count + unsafe block count vs SQLite.
   - **Table III**: telemetry totals — total runs, failures, lost-acked-commits, integrity check status.
8. **VII. Discussion.** Where Redline still trails: hot-row contention (SQLite's WAL writer batching wins), range scans (range-cursor needs prefetch), single-thread per-tx overhead. Future work: vectorized executor, encryption-at-rest, libsql migration, range-scan prefetch.
9. **VIII. Conclusion.** Restate measured wins, the SQLite-compatibility footprint, the deterministic-crash story, and link to the open artifact.
10. **Appendix.** Architecture diagram (full size), bench command lines, manifest excerpt.
11. **References.** ~30–40 entries: SQLite design docs, ARIES, B-link tree, MVCC literature, Rust DB ecosystem, libSQL/Turso, sled, redb, fail-rs, etc.

## Production phases

### Phase A — Data + figures (parallel, 1 agent)

Owner: **Agent FIG**.

- Read `target/bench/xbabe1/certification/{runs.jsonl, summary.csv, manifest.json}`.
- Read `target/bench/wave7-{compat,recovery,failpoint}.json`.
- Compute per-(engine, workload, durability, threads) **mean + stddev** of qps + p99 across the 5 reps.
- Compute Redline/SQLite ratios.
- Compute LOC by running `cloc` (or hand-rolled `find ... -name '*.rs' | xargs wc -l`) on `crates/`. Compare to SQLite `~250 KLOC` published number.
- Generate the following EPS figures via matplotlib (`backend = ps`, `font.family = serif`, `font.size = 9`, `figure.figsize = (3.4, 2.4)` for two-column, label fonts via `mathtext.fontset = stix`):
  - `fig1_throughput_scaling.eps` — log-y throughput vs threads, 5 representative workloads (point-read-pk, mixed-95-5, mixed-50-50, writers-disjoint, secondary-index-read), Redline solid, SQLite dashed, color-coded per workload.
  - `fig2_latency_p99.eps` — p99 latency vs threads, same workloads.
  - `fig3_ratio_bars.eps` — grouped bar chart of Redline/SQLite ratio per (workload, thread). Highlight bars where ratio ≥ 4×.
  - `fig4_scaling_efficiency.eps` — qps(threads)/qps(1) vs threads, with `y = x` ideal line.
  - `fig5_recovery_failpoint.eps` — bar chart of recovery-matrix (36/36) and failpoint-matrix (24/24) PASS counts per category.
- Generate the following data files under `paper/data/`:
  - `headline_table.csv` — for Table I.
  - `loc_comparison.csv` — for Table II.
  - `cert_totals.csv` — for Table III.
- Acceptance: every EPS opens cleanly in `gs`, every data file has correct column counts, ratio computation cross-checks the inline numbers shown to the user during cert.

### Phase B — Architecture diagram (parallel with A, 1 agent)

Owner: **Agent ARCH**.

- Author the architecture diagram as a TikZ picture stored in `paper/figs/architecture.tex` AND a standalone `paper/figs/architecture.eps` rendering (via `latex` + `dvips` + `ps2eps`).
- Diagram contents:
  - 5 horizontal stacked layers: **C ABI shim (sqlite3.h) | rldb FFI | redlinedb facade | redlinedb-sql (parser/planner/exec) | redlinedb-kernel (catalog, index, engine, wal, storage)**.
  - Side annotations: `unsafe` boundary; failpoint sites (WAL/commit/heap/index/catalog); MVCC layer.
  - Bottom: durable storage (page heap, WAL segments, catalog snapshot, control files).
- Acceptance: EPS renders at print quality, layers + arrows are legible at column width.

### Phase C — Bibliography (parallel with A/B, light)

Owner: **Agent BIB**.

- Build `paper/refs/refs.bib` with at minimum:
  - SQLite official documentation + the Hipp-Wirzenius "Architecture of SQLite" technical report.
  - Mohan et al. ARIES (1992).
  - Lehman & Yao B-link tree (1981).
  - Bernstein/Goodman MVCC chapter.
  - Reed snapshot isolation.
  - Berenson et al. ANSI isolation critique.
  - Rust Programming Language reference (Klabnik).
  - libSQL / Turso whitepaper (cite their public docs).
  - sled / redb design notes.
  - fail-rs (Apache failpoint library).
  - rusqlite docs.
  - sqllogictest (Hipp).
  - JEPSEN reports for SQLite-class systems.
  - PostgreSQL MVCC docs (cite Stonebraker original Postgres paper).
  - InnoDB MVCC docs.
  - FoundationDB SOSP paper for deterministic simulation testing.
  - SQLite test corpus (sqlite.org/testing.html).
  - LMDB Howard Chu paper.
  - LevelDB / RocksDB short cites.
  - HyPer / DuckDB papers (for embedded analytical context).
- Aim for **30–40** total references.
- Acceptance: `bibtex` runs clean against the writer's `\cite{}` calls.

### Phase D — Writing (sequential, single strong writer)

Owner: **Agent SCRIBE**.

- Inputs: figures from FIG, diagram from ARCH, refs from BIB, this PLAN.md, `docs/WORKPLAN_slam.md` ledger.
- Output: `paper/main.tex` using `\documentclass[conference]{IEEEtran}`.
- Structure follows the section list above. Tone: neutral, honest, comparative; no hand-waving wins.
- Inline tables formatted with `\begin{tabular}` (small font for fit). Tables I-III sourced from the CSVs in `paper/data/`.
- Code excerpts (≤ 8 lines each, monospace listings via `listings`):
  - WAL coordinator commit hot path.
  - Per-tx `IndexUndoOp` enum.
  - `try_match_index_access` planner gate.
  - Failpoint macro (compile-neutral when off).
- All section bodies in `paper/sections/*.tex` (intro.tex, background.tex, architecture.tex, implementation.tex, methodology.tex, evaluation.tex, discussion.tex, conclusion.tex), `\input{...}` from `main.tex`.
- Acceptance: every figure cited at least once, every reference cited at least once, page count between 8 and 10 inclusive.

### Phase E — Compile + verify (sequential)

Owner: **Agent BUILD**.

- `cd paper && pdflatex -output-directory=build main.tex && bibtex build/main && pdflatex -output-directory=build main.tex && pdflatex -output-directory=build main.tex` (or `latexmk -pdf`).
- Verify: page count 8–10, every `\ref{}` resolves, every `\cite{}` resolves, no `Overfull \hbox` > 5pt, every figure embedded, font is consistent.
- Output: `paper/build/main.pdf` plus `paper/main.pdf` symlink/copy for convenience.
- Acceptance: PDF opens cleanly in any reader, all figures render, references list is non-empty and well-formed.

### Phase F — Final polish

- Visual review of the PDF.
- Small wording fixes (typos, formatting).
- Final commit with `paper/main.pdf`, `paper/main.tex`, `paper/sections/*`, `paper/figs/*.eps`, `paper/data/*.csv`, `paper/refs/refs.bib`.
- Tag `paper-v1`.

## File layout (final state)

```
paper/
├── PLAN.md                         (this file)
├── REQUIREMENTS.md                 (toolchain, fonts, etc.)
├── main.tex                        (front matter + \input glue)
├── main.pdf                        (camera-ready)
├── sections/
│   ├── abstract.tex
│   ├── introduction.tex
│   ├── background.tex
│   ├── architecture.tex
│   ├── implementation.tex
│   ├── methodology.tex
│   ├── evaluation.tex
│   ├── discussion.tex
│   ├── conclusion.tex
│   └── appendix.tex
├── figs/
│   ├── architecture.tex            (TikZ source)
│   ├── architecture.eps            (rendered)
│   ├── fig1_throughput_scaling.eps
│   ├── fig2_latency_p99.eps
│   ├── fig3_ratio_bars.eps
│   ├── fig4_scaling_efficiency.eps
│   └── fig5_recovery_failpoint.eps
├── data/
│   ├── headline_table.csv
│   ├── loc_comparison.csv
│   └── cert_totals.csv
├── refs/
│   └── refs.bib
└── build/
    └── (latexmk output: main.aux, main.bbl, main.log, main.pdf)
```

## Acceptance gate (paper is "done" when ALL true)

- [ ] `paper/main.pdf` exists, page count 8–10
- [ ] All 6 figures present and EPS-validated
- [ ] All 3 tables populated from `paper/data/*.csv`
- [ ] ≥ 30 refs in `refs.bib`, all cited at least once in main.tex
- [ ] No undefined `\ref` or `\cite` in `build/main.log`
- [ ] LOC table cites a real `cloc` or `wc -l` count from this repo (no fabricated numbers)
- [ ] Headline numbers in evaluation match the live `target/bench/xbabe1/certification/runs.jsonl` (cross-checked against the user-facing summary I posted)
- [ ] Tag `paper-v1` exists on main
- [ ] `docs/WORKPLAN_slam.md` records the paper artifacts (path + SHA-256 of main.pdf)

## Risk register

- **Toolchain gaps.** TeX Live + IEEEtran are present; matplotlib + pandas confirmed. If a TikZ→EPS render fails, fall back to PDF-only embedding (`\includegraphics{architecture.pdf}`); IEEEtran tolerates either format.
- **Page overrun.** Tighten methodology and discussion first. Trim related-work to a tight survey.
- **Latex errors.** First compile pass may surface citation key typos; build agent reruns until clean.
- **Number mismatch.** Cross-check every number in the paper against `runs.jsonl`; never paraphrase from memory.
