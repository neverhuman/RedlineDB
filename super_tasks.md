# RedlineDB Performance Super Tasks

## Summary

- Current official evidence: `1127/1127` SQLite-parity cases pass, median latency gap is `-19.69%`, worst gap is `-289.42%`, and `57` cases are over `2x` slower than SQLite.
- The biggest remaining wins are fixed CLI/database-open cost, executor materialization, grouped/window aggregate algorithms, recursive CTE cloning/dedup, JSON parse churn, and index/order/limit pushdown.
- Several helper ideas are already partly landed: CLI uses `mimalloc`, stdout and `.once` are buffered, delimited output streams, and `.read` uses mmap. The tasks below avoid duplicating those and target the remaining gaps.

## Ranked Tasks

### 1. ShellZero / Pre-Open CLI Fast Path

**Why:** Many scalar, dot-command, and tempfile cases pay for `clap`, `Database::create_in_memory`, `connect`, statement cache, page cache, and SQL runtime even when the workload is pure shell state or fromless scalar SQL. This directly hits `SCALAR_ARITH_*`, `CLI_TEMPFILE`, `.cd`, `.print`, `.output`, `.once`, `.read`, and `-append SELECT 1` cases.

**Change:** Add a pre-open executor before the current DB open in `crates/cli/src/lib.rs`. It should accept only audited shell-local commands and fromless scalar `SELECT` expressions; any storage-inspecting command falls back to the existing path.

**Files:** `crates/cli/src/lib.rs`, `crates/cli/src/dot/mod.rs`, `crates/cli/src/render.rs`, `crates/cli/tests/shell_fast_path.rs`.

```rust
enum PreOpenDecision {
    Handled(i32),
    Fallback,
}

fn try_preopen_fast_path(
    cli: &Cli,
    raw_args: &[String],
    stdin: Option<&str>,
) -> Result<PreOpenDecision, String> {
    let program = collect_cli_program(cli, stdin)?;
    let mut shell = TinyShellState::from_flags(raw_args)?;
    for stmt in preflight(&program, 0)? {
        match stmt {
            TinyStmt::Dot(cmd) if cmd.is_shell_local() => shell.run_dot(cmd)?,
            TinyStmt::FromlessSelect(exprs) => shell.write_scalar_row(eval_const_exprs(exprs)?)?,
            _ => return Ok(PreOpenDecision::Fallback),
        }
    }
    shell.flush()?;
    Ok(PreOpenDecision::Handled(0))
}
```

**Proof:** Existing CLI shell tests plus targeted cases `00146`, `00148`, `00149`, `00153`, `00202`, `00376`; then `rtk just cli-test` and the verified external `rtk just redline-testing-official` gate.

### 2. Lean Ephemeral Defaults and Lazy Spill Roots

**Why:** Process RSS is dominated by default caches and eager temp setup. Current defaults include a `16 MiB` public cache, `8 MiB` query work memory, `128` statement cache entries, and `QueryMemoryBroker::new` always creates the temp root.

**Change:** Add a `lean_ephemeral` open profile for CLI `:memory:`/empty/private DBs: small page cache, small statement cache, smaller `work_mem`, lower shard counts, and lazy temp-directory creation only when a spill file is actually allocated.

**Files:** `crates/redlinedb/src/options.rs`, `crates/redlinedb/src/handle.rs`, `crates/sql/src/connection/options.rs`, `crates/sql/src/batch.rs`, `crates/cli/src/lib.rs`.

**Code shape:** `OpenOptions::lean_ephemeral()` should preserve public defaults; only CLI/private-memory paths opt in. `QueryMemoryBroker` should store `Option<PathBuf>` and create dirs in `ensure_spill_file`, not in `new`.

**Proof:** `rtk cargo test -p redlinedb --test phase11_ephemeral --quiet --locked`, `rtk just sql-test`, top RSS/latency memory cases, and `/usr/bin/time -v` before/after for scalar and aggregate cases.

### 3. Complete File I/O Streaming

**Why:** `.output FILE` still stores a raw `File`, `write_readonly_sidecar` uses the same unbuffered path, and `SELECT hex(readfile(...))` still reads the whole file and builds a full hex `String`.

**Change:** Make every file sink a `BufWriter<File>` with append/truncate helpers, and stream `hex(readfile())` through a fixed buffer plus hex lookup table.

**Files:** `crates/cli/src/dot/mod.rs`, `crates/cli/src/dot/io_cmd.rs`, `crates/cli/src/lib.rs`.

```rust
const HEX: &[u8; 16] = b"0123456789ABCDEF";

fn write_hex_file<W: Write>(path: &Path, out: &mut W) -> Result<(), String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 64 * 1024];
    let mut hex = [0u8; 128 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        for (i, b) in buf[..n].iter().enumerate() {
            hex[2 * i] = HEX[(b >> 4) as usize];
            hex[2 * i + 1] = HEX[(b & 0x0f) as usize];
        }
        out.write_all(&hex[..2 * n]).map_err(|e| e.to_string())?;
    }
    Ok(())
}
```

**Proof:** Cases `00148`, `00149`, `00150`, `00151`, `00156`, `00202`, plus `rtk just cli-test`.

### 4. One-Pass Grouped Aggregation

**Why:** `execute_grouped_select` groups into `Vec<Vec<SqlRow>>`, then projection/HAVING can rescan each group per aggregate. Worst official case is `00566 AGG_GROUP_HAVING_059` at `3.89x` SQLite latency.

**Change:** Replace group materialization for built-in aggregates with a one-pass state table: encode group key once, update all aggregate slots per row, then evaluate HAVING/projection from accumulator slots. Keep fallback for UDF aggregates, exotic aggregate clauses, and unsupported DISTINCT forms.

**Files:** `crates/sql/src/exec/agg/group.rs`, `crates/sql/src/exec/agg_eval.rs`, `crates/sql/src/exec/vec/hash_agg.rs`.

```rust
struct GroupState {
    first: SqlRow,
    accs: SmallVec<[AggState; 8]>,
    distinct: SmallVec<[DistinctSet; 2]>,
}

for row in filtered {
    key_scratch.clear();
    encode_group_key_into(&plan.group_by, &row, bindings, &mut key_scratch)?;
    let state = groups
        .entry(key_scratch.clone())
        .or_insert_with(|| GroupState::new(&row, &agg_plan));
    for slot in &agg_plan.slots {
        if slot.filter_passes(&row, bindings)? {
            state.accs[slot.id].step(slot.arg(&row)?)?;
        }
    }
}
```

**Proof:** Aggregate parity tests, cases `00566`, `00509`, all `GEN_SQL_AGGREGATE`, and `rtk just sql-test`.

### 5. Window Engine Linearization

**Why:** `evaluate_window_functions` builds `Vec<Vec<Vec<SqlValue>>>`, caches layouts by debug strings, and generic aggregate frames rebuild accumulators per output row. Worst window case `00834` is `3.0x` SQLite latency.

**Change:** Add structural `WindowLayoutKey`, whole-partition aggregate fast path, prefix arrays for `ROWS UNBOUNDED PRECEDING..CURRENT ROW`, inverse accumulators for bounded `ROWS`, and direct projection output without per-call result cubes.

**Files:** `crates/sql/src/exec/expr/window_eval.rs`, `crates/sql/src/exec/expr/window_eval/{partition.rs,accumulator.rs,frame.rs}`.

**Code shape:** For supported aggregate windows, produce a single `Vec<SqlValue>` per output expression from one partition scan; fallback to current generic evaluator only for unsupported `RANGE/GROUPS/EXCLUDE` combinations.

**Proof:** Window tests plus cases `00806`, `00809`, `00834`, `00859`; then `rtk cargo test -p redlinedb-sql --test parity_window --quiet --locked` if present, otherwise `rtk just sql-test`.

### 6. Recursive CTE Queue Worktable

**Why:** `cte_recursive.rs` clones `accumulated` and `working_set` repeatedly, synthesizes table defs every iteration, and uses linear `row_in` dedup. `CTE_RECURSIVE_MATRIX_*` is among the worst classes.

**Change:** Store rows in an arena/worktable, keep queue indexes instead of cloning row vectors, and use encoded row-key hash sets for `UNION` dedup.

**Files:** `crates/sql/src/exec/cte_recursive.rs`, `crates/sql/src/exec/cte_registry.rs`, `crates/sql/src/exec/cte.rs`.

**Proof:** Cases `00713`, `00786`, `SQL_CTE`, recursive CTE tests, and `rtk just sql-test`.

### 7. Index, ORDER BY, LIMIT, and `INDEXED BY` Contract

**Why:** Current planner handles some index range/count/covering cases but strips `INDEXED BY` before planning, skips expression indexes, lacks reverse cursor support for DESC, and DML rejects `ORDER BY/LIMIT`.

**Change:** Preserve table access directives in the bound table source, enforce `INDEXED BY` as a hard no-fallback contract, support `NOT INDEXED`, add reverse/raw cursor scan, composite ORDER BY satisfaction, expression-index equality matching, and DML candidate rowid selection with ORDER/LIMIT.

**Files:** `crates/sql/src/parser/prepare.rs`, `crates/sql/src/parser/helpers/table/*`, `crates/sql/src/statement.rs`, `crates/sql/src/planner/access.rs`, `crates/sql/src/exec/{index_access.rs,index_batch.rs,select_top.rs,tail.rs,tail_build.rs}`.

```rust
struct TableAccessHint {
    indexed_by: Option<DbName>,
    not_indexed: bool,
}

fn dml_target_rowids(spec: &DmlTargetSpec, tx: &mut Txn) -> Result<Vec<RowId>> {
    if let Some(path) = match_ordered_index(spec)? {
        return scan_index_rowids(path, spec.limit_plus_offset(), spec.direction).map(apply_offset);
    }
    topk_heap_over_heap_scan(spec)
}
```

**Proof:** Existing index tests, new hard-failure tests for missing `INDEXED BY`, generated cases `00429`, `00493`, `01072`, `01085`, plus `rtk just sql-test`.

### 8. JSON1 Fast Path Through JSONB Bytes

**Why:** JSON cases are now top outliers (`01058` is `3.83x` SQLite). The SQL JSON1 layer repeatedly uses `serde_json::Value`; the kernel already has JSONB encoding and path bytecode that can evaluate paths over bytes.

**Change:** Cache parsed JSONB bytes and compiled paths per statement/thread, route `json_extract`, `json_type`, `json_array_length`, and `json_valid` through byte-level JSONB where semantics match, and only inflate to `serde_json::Value` for mutators like `json_set`.

**Files:** `crates/sql/src/json/scalar.rs`, `crates/kernel/src/json/{encode.rs,decode.rs,path_bytecode.rs}`, `crates/sql/src/exec/expr/json_dispatch.rs`.

**Proof:** JSON tests, cases `01011`, `01058`, all `GEN_SQL_JSON`, and `rtk just sql-test`.

### 9. Arena Rows, Borrowed Values, and Expression Bytecode

**Why:** Many hot paths clone `Vec<SqlValue>`, convert `ValueRef` to owned `Arc`, and evaluate AST nodes directly. This drives both latency and RSS across scalar, joins, views, constraints, and sorting.

**Change:** Introduce statement arenas and `SmallVec<[SqlValue; 8]>` rows, add `RowView/ValueRef` projection for simple scans, and compile scalar expressions into compact bytecode with reusable stacks.

**Files:** `crates/sql/src/exec/expr/*`, `crates/sql/src/exec/expr/scalar/row/*`, `crates/sql/src/exec/select_top.rs`, `crates/sql/src/batch.rs`.

**Proof:** Full `rtk just sql-test`, allocation-budget tests for top memory cases, and the verified external `rtk just redline-testing-official` gate.

### 10. B-Link Split Path Without Global Structure Lock

**Why:** The index layer has per-page latches and right links, but leaf splits still serialize through `structure_lock`. This matters less for one-shot parity latency, but it is a major beyond-SQLite concurrency ceiling.

**Change:** Replace split serialization with a B-link protocol: latch-coupled descent, right-link/high-key validation, atomic sibling publication, parent propagation with retry, and root-split CAS/short meta latch. Keep crash recovery page-image rules intact.

**Files:** `crates/kernel/src/index/{mod.rs,mutate/insert.rs,maintenance/split.rs,cursor.rs,latches.rs}`, recovery/failpoint tests under `crates/kernel`.

**Proof:** `rtk just kernel-test`, failpoint matrix for index split crashes, `rtk cargo run -p redlinedb-bench --release -- certify --config crates/bench/bench/phase11-oltp-gap.toml --out-dir target/bench/phase11-oltp-gap --seed 7 --repetitions 3 --warmup 1`.

### 11. Native-Max Build Lane: PGO + BOLT + CPU Tiers

**Why:** The repo already has fat LTO, `target-cpu=native`, mold, and PGO scaffolding. Add the missing post-link and CPU-tier automation after algorithmic changes land.

**Change:** Add `scripts/perf/bolt.sh`, extend `scripts/perf/pgo.sh` training to quick+medium+phase11 workloads, and document CPU-specific artifacts (`x86-64-v3`, `x86-64-v4`, `neoverse`) without changing correctness surfaces.

**Files:** `Cargo.toml`, `.cargo/config.toml`, `scripts/perf/{pgo.sh,lib-rustflags.sh,bolt.sh}`, `docs/performance.md`, `just/lanes.just`.

**Proof:** Binary smoke tests, retained stdout/stderr hash parity, and the verified external `rtk just redline-testing-official` gate.

## Validation Plan

- Always run targeted crate lanes first: `rtk just cli-test`, `rtk just sql-test`, or `rtk just kernel-test` depending on task ownership.
- For performance PRs, require top-case before/after rows for: `00566`, `01058`, `00493`, `01085`, `00786`, `00834`, `00376`, `00146`, `00148`, `00149`, `00202`.
- Use targeted Rust tests and binary smoke checks for iteration; use `rtk just redline-testing-official` for the PR gate and `perf-full` before claiming corpus-wide performance wins.
- Add allocation/RSS budget tests for memory-profile cases; report process RSS and baseline-adjusted/query incremental RSS separately.

## Expected Outcome

- Conservative full-plan estimate: median parity latency moves from about `1.20x` SQLite to `0.85x-1.00x` SQLite, with worst current outliers reduced from `3.9x` slower to roughly `1.2x-1.8x`.
- Query-level allocations should drop `3x-10x` on window/aggregate/CTE paths; total process RSS will not honestly match SQLite's tiny shell RSS until ShellZero/tiny-CLI paths avoid opening the Rust engine.
- B-link and native-max build work should add separate beyond-SQLite wins: `2x-10x` under indexed concurrent writes, and `5%-20%` latency improvement from PGO/BOLT/CPU-tier builds after the algorithmic changes are stable.

## Assumptions

- This plan is intended to become `/home/ubuntu/redtemp/RedlineDB/super_tasks.md`.
- No repo files were changed during the planning pass.
- The helper directory had `.md`/`.diff` files, not `.txt`; all files in `tips/performance/helper/` were treated as the requested helper input.
