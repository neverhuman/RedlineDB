# RedlineDB Branch Recovery Ledger

Status: W1 audit artifact, initial pass complete.

Owner: Codex.

Controlling plan: `speed_up_workplan_FINAL.md`, section 5, W1.

Baseline dependency: no source ports from this ledger should land until W0
publishes the frozen v4.0.9 evidence bundle.

## Rules

- No divergent branch is a merge target.
- Every source port must be isolated to a topical commit or a manual patch.
- Every port needs targeted conformance first, then a before/after perf case list.
- Every semantic change needs a rollback boundary commit.
- Generated zones are not edited by this ledger.

## Audit Commands

Commands were run from `/home/ubuntu/redlineDB` on `HEAD`
`08d44d0f883a9cabdc13b4d6326740a87bfe6b8e`
(`Merge branch 'codex/rql-phase1-ci-restore' into 'main'`).

```bash
rtk git rev-parse --short HEAD
rtk git show -s --format='%H%n%ci%n%s' HEAD
rtk git branch -a --list '*claude-gap-closure*' '*perf/parity-gap-closure*' '*track-*' '*preserve/redlinedb-sql-cli-runtime-20260524*' '*rql*'
rtk bash -lc 'for b in origin/perf/parity-gap-closure claude-gap-closure track-a-scalars track-b-types track-e-cli track-f-jsonb track-k-portability-syntax preserve/redlinedb-sql-cli-runtime-20260524 rql; do if git rev-parse --verify --quiet "$b" >/dev/null; then mb=$(git merge-base HEAD "$b"); ahead=$(git rev-list --count HEAD.."$b"); behind=$(git rev-list --count "$b"..HEAD); printf "%s|merge_base=%s|unique=%s|head_only=%s\n" "$b" "$mb" "$ahead" "$behind"; fi; done'
rtk bash -lc 'for b in origin/perf/parity-gap-closure claude-gap-closure track-a-scalars track-b-types track-e-cli track-f-jsonb track-k-portability-syntax preserve/redlinedb-sql-cli-runtime-20260524 rql; do if git rev-parse --verify --quiet "$b" >/dev/null; then printf "BRANCH %s\n" "$b"; git diff --shortstat HEAD..."$b"; git diff --name-only HEAD..."$b" | sed -n "1,80p"; fi; done'
rtk git log --oneline --no-merges HEAD..origin/perf/parity-gap-closure
rtk git log --oneline --no-merges HEAD..claude-gap-closure --reverse
rtk git log --oneline --no-merges HEAD..track-a-scalars --reverse
rtk git log --oneline --no-merges HEAD..track-b-types --reverse
rtk git log --oneline --no-merges HEAD..track-e-cli --reverse
rtk git log --oneline --no-merges HEAD..track-f-jsonb --reverse
rtk git log --oneline --no-merges HEAD..track-k-portability-syntax --reverse
rtk git log --oneline --no-merges HEAD..preserve/redlinedb-sql-cli-runtime-20260524 --reverse
rtk git log --oneline --no-merges HEAD..rql --reverse
rtk git diff --stat HEAD origin/perf/parity-gap-closure -- crates/sql/src/exec/cte_recursive.rs crates/sql/src/exec/index_access.rs crates/sql/src/parser/prepare.rs crates/sql/src/exec/hot_row.rs crates/sql/src/exec/select_top.rs
rtk git diff --stat HEAD rql -- crates/sql/src/rql.rs crates/redlinedb/tests/rql.rs crates/cli/tests/rql.rs crates/cli/src/lib.rs crates/redlinedb/src/connection.rs crates/redlinedb/src/handle.rs crates/sql/src/connection/session.rs
```

## Branch Summary

| Branch | Merge base | Unique commits vs HEAD | HEAD-only commits | Ledger decision |
|---|---|---:|---:|---|
| `origin/perf/parity-gap-closure` | `27132c4a338451570f8d83ed36c6efd9ed9d48e8` | 37 | 75 | `already-in-main` for Phase 5 speed work; do not merge |
| `claude-gap-closure` | `c92b1d3e508f51ad75861ca4385c25a0abc9860d` | 87 | 79 | `reject` wholesale; mine only already-present or benchmark-gated slices |
| `track-a-scalars` | `c92b1d3e508f51ad75861ca4385c25a0abc9860d` | 3 | 79 | `already-in-main` by squashed content |
| `track-b-types` | `c92b1d3e508f51ad75861ca4385c25a0abc9860d` | 3 | 79 | `already-in-main` for profile pieces; semantic pieces not W1 speed ports |
| `track-e-cli` | `c92b1d3e508f51ad75861ca4385c25a0abc9860d` | 4 | 79 | `already-in-main` / W7-owned for any remaining CLI benchmark work |
| `track-f-jsonb` | `c92b1d3e508f51ad75861ca4385c25a0abc9860d` | 11 | 79 | `already-in-main` for JSONB surface; no wholesale port |
| `track-k-portability-syntax` | `c92b1d3e508f51ad75861ca4385c25a0abc9860d` | 51 | 79 | `reject` wholesale; HPC slices already present |
| `preserve/redlinedb-sql-cli-runtime-20260524` | `c020508247e598d412b2b65c9c8e36b0ae63afab` | 1 | 82 | `needs-benchmark`; high-value but high-conflict slices |
| `rql` | `a9a7d88b4ff6e90306302273ffbef03aa1254c7e` | 1 | 71 | `already-in-main`; W3 native RQL is new work, not a branch port |

Sub-agent cross-checks:

- `claude-gap-closure` and `origin/claude-gap-closure` both resolved to
  `79aa0f92e386` during the Codex explorer audit. The branch has 87 unique
  commits vs `HEAD`: 75 non-merge commits and 12 merge commits. A direct
  two-dot diff from `HEAD` to the branch is stale and destructive relative to
  current `main` (`246 files changed, 8738 insertions(+), 72721 deletions(-)`),
  so the branch is not a safe merge or cherry-pick source.
- `track-a-scalars`, `track-b-types`, `track-e-cli`, `track-f-jsonb`, and
  `track-k-portability-syntax` all show patch-unique branch commits by
  `git cherry -v`, but the low-risk performance topics are semantically present
  or superseded on current `HEAD`.
- `preserve/redlinedb-sql-cli-runtime-20260524` is one commit at
  `a6b1bc1985e3` and touches 32 files; 26 overlap files changed on `HEAD`
  since merge-base. It is high-conflict and useful only as an idea source.
- `rql` and `origin/rql` both resolve to `22df8974feb3`; core runtime and test
  blobs are already present on `HEAD`.

## Candidate Ledger

| Candidate | Source | Files / evidence | Status | Risk | Next action |
|---|---|---|---|---|---|
| Phase 5 parity gap closure bundle | `origin/perf/parity-gap-closure`, commits `7e57fb4`, `6704b1e`, `67e095c`, `79bf016`, `06fe6b2` | `cte_recursive.rs`, `index_access.rs`, and `parser/prepare.rs` have matching line counts and no diff against `HEAD`; `hot_row.rs` and `select_top.rs` are larger on `HEAD` | `already-in-main` | Low | Skip branch. Use current `main` code as source of truth. |
| Parser rewrite allocation reduction | `origin/perf/parity-gap-closure` commit `b62d4ad` | `crates/sql/src/parser/prepare.rs` is byte-equivalent by zero diff; `crates/sql/src/parser.rs` already has cheap ASCII scans and targeted lowercasing | `already-in-main` | Low | No port. |
| Function-name lowercase caching | `origin/perf/parity-gap-closure` commit `4a89e9a`; related `d348e0b` | Current scalar and aggregate paths already use cached/lowercase helpers; `crates/sql/src/exec/agg_eval.rs` has thread-local `ahash` caches | `already-in-main` | Low | No port. |
| Fromless SELECT fast path | `origin/perf/parity-gap-closure` commit `2e13dc5` | `crates/sql/src/exec/select_top.rs` contains `try_fromless_select_fast_path` and fromless comments | `already-in-main` | Low | No port. |
| Statement cache AHash | `origin/perf/parity-gap-closure` commit `a20de92`; `claude-gap-closure` commit `11d716a` | `crates/sql/src/connection/cache.rs` uses `ahash::RandomState`; `crates/sql/Cargo.toml` depends on `ahash` | `already-in-main` | Low | No port. |
| Hot SQL maps AHash | `claude-gap-closure` commit `11d716a`; `track-k-portability-syntax` commit `11d716a` | Current `agg_eval`, `vec/hash_agg`, `morsel/hash_agg`, `cte_recursive`, and catalog stats/schema use `ahash` | `already-in-main` | Medium | W2 may benchmark allocator/hash choices, but no branch port needed. |
| String interning for hot identifiers | `claude-gap-closure` / `track-k-portability-syntax` commit `6c4ac7e` | `crates/sql/src/exec/intern.rs` exists and row lookup uses `intern_arc` | `already-in-main` | Medium | No port. W2 can measure if cache size needs tuning. |
| ASCII `LENGTH`/`UPPER`/`LOWER` and memmem `INSTR` | `origin/perf/parity-gap-closure` commit `5bbe650`; related `9abab6c` | `crates/sql/src/exec/expr/json_dispatch.rs` contains memmem `instr`; scalar value helpers are present | `already-in-main` | Low | No port. |
| CLI integer formatting via `itoa` | `origin/perf/parity-gap-closure` commit `32e078d` | `crates/cli/src/render.rs` uses `itoa::Buffer`; `crates/cli/Cargo.toml` depends on `itoa` | `already-in-main` | Low | No port. |
| SQLite-style REAL formatter | `track-a-scalars` commit `a3e492c`; `track-b-types` commit `2c4cd3b` | `crates/sql/src/exec/expr/scalar/value.rs` exposes `format_real_sqlite`; CLI delegates through `redlinedb::format_real_sqlite` | `already-in-main` | Low | No port. |
| SQL math scalar/libm surface | `track-a-scalars` commit `d30a08c`; `claude-gap-closure` commits `f011a66`, `2bbb0db` | Current `math.rs` and `json_dispatch.rs` route math functions through `libm`; `crates/sql/Cargo.toml` depends on `libm` | `already-in-main` | Low | No port. |
| GLOB parser rewrite | `track-a-scalars` commit `9d4d08e` | `crates/sql/src/parser.rs` has `rewrite_glob_operators`; `parity_scalar_funcs.rs` covers `GLOB` and `NOT GLOB` | `already-in-main` | Low | No port. |
| Build profiles and PGO script | `track-b-types` commit `fc782f2`; `track-f-jsonb` commit `ccef45a` | `Cargo.toml` has `release-native` and `release-pgo`; `scripts/perf/pgo.sh` exists; `docs/performance.md` documents both | `already-in-main` | Medium | W2 should extend to `release-pgo-bolt` and allocator matrix, not port old branch verbatim. |
| Build profile documentation drift | Current `HEAD`, found while comparing `track-b-types` | `docs/performance.md` says release inherits `lto = "thin"` while `Cargo.toml` sets release `lto = "fat"` | `needs-benchmark` / docs fix | Low | Fix in W2/A6 profile audit, after Claude's A6 handoff decision. |
| CLI output / dot-command parity | `track-e-cli` commits `b1f46d2`, `3580db0`, `69c7475`, `ee2a131` | Current CLI has output mode, dot-command, option, and RQL rendering paths; tests exist under `crates/cli/tests` | `already-in-main` / W7-owned | Medium | Do not port in W1. Claude W7 owns any remaining startup/rendering changes. |
| JSONB operators and TVFs | `track-f-jsonb` commits `0b84e73`, `f631972`, `85a18a6`, `44ba8ea` | Current `crates/sql/src/json/jsonb.rs`, `json_tv.rs`, parser JSONB question-op rewrites, and JSONB tests exist | `already-in-main` | Medium | No W1 port. Future JSON perf work needs W0-ranked case proof. |
| Track K portability syntax | `track-k-portability-syntax` commits `906211a`, `7d4bd33`, `d4a5fa7`, `15131ef`, `32e33b0`, `a809607`, `d1ebfb3` | Current parser and tests contain Track K `FETCH FIRST`, `DISTINCT ON`, `MERGE`, `LATERAL`, and grouping-set support | `reject` for W1 perf | High | No branch port. These are semantic surfaces, not speed-recovery shortcuts. |
| Mimalloc global allocator | `track-k-portability-syntax` / `claude-gap-closure` commit `1b72067` | `crates/cli/Cargo.toml` defaults to `alloc-mimalloc`; `crates/cli/src/main.rs` and `bin/redlinedb-cli.rs` install the allocator | `already-in-main` | Medium | W2 should benchmark system vs jemalloc vs mimalloc under SQL/RQL/RSS; no branch port. |
| Simple aggregate runtime | `preserve/redlinedb-sql-cli-runtime-20260524` commit `a6b1bc1` | Adds `crates/sql/src/exec/agg/simple.rs` (964 LOC) and routes it before current grouped aggregation | `needs-benchmark` | High | Do not cherry-pick. Mine a narrowed scalar/no-`GROUP BY` or simple grouped subset only after W0, with SQLite oracle tests for NULL, DISTINCT, HAVING, ORDER BY, overflow, empty input, and group semantics. |
| Predicate runtime / subquery caches | `preserve/redlinedb-sql-cli-runtime-20260524` commit `a6b1bc1` | Large rewrite in `crates/sql/src/exec/expr/predicate.rs`; includes `TablePredicate`, simple comparison/modulo fast path, `FastExistsPlan`, and alternate IN cache | `needs-benchmark` | High | Mine only small safe slices after W0. Preserve version would overwrite current scalar-subquery/bool semantics if taken wholesale. |
| Top-K tiny buffer | `preserve/redlinedb-sql-cli-runtime-20260524` commit `a6b1bc1` | Modifies `crates/sql/src/exec/vec/topk.rs` with `TopKBuffer`; `select_top.rs` adds `would_admit` projection skip | `needs-benchmark` | Medium | Best preserve candidate, but port only the narrow `topk.rs` piece first. Add targeted tests for NULL direction, ties, aliases, LIMIT 0/1/64/65, and OFFSET. |
| CLI runtime changes from preserve | `preserve/redlinedb-sql-cli-runtime-20260524` commit `a6b1bc1` | Touches `crates/cli/src/lib.rs`, `.dot` I/O, and `crates/redlinedb/src/registry.rs`; unique ideas include readonly-sidecar freshness and generic `dump_database<W>` | W7-owned / mostly `reject` | Medium | Coordinate with Claude before touching. Speed pieces are mostly superseded by ShellZero, process-owner-lock changes, buffered output, and batched `.import` on `HEAD`. |
| JSON scalar cache changes from preserve | `preserve/redlinedb-sql-cli-runtime-20260524` commit `a6b1bc1` | Replaces current JSONB byte fast path with text/path HashMap cache in `crates/sql/src/json/scalar.rs` | `reject` as-is | High | Do not port. It appears to remove current JSONB byte-path work. |
| RQL phase 1 support | `rql` commit `22df897` | `git diff --stat HEAD rql -- rql paths` is empty; `crates/sql/src/rql.rs` is 1231 LOC on `HEAD` | `already-in-main` | Low | W3 native RQL is new implementation work, not branch salvage. |

## Highest-Value Future Ports

These are the only W1 findings worth implementing after W0 evidence exists:

1. Preserve-branch top-k tiny buffer, benchmarked on ORDER BY LIMIT cases.
2. Preserve-branch simple aggregate runtime, manually reworked behind tests.
3. Preserve-branch predicate/subquery cache ideas, narrowed to proven
   uncorrelated IN and EXISTS short-circuit shapes.

Everything else found in the audited branches is either already present on
`main`, semantic/conformance work outside the speed-recovery critical path, or
too broad to port safely.

## W2 Follow-Ups From W1

The audited branches do not provide clean W1 ports, but they identify W2
measurement axes that should be carried into the build/perf matrix:

- allocator matrix: system vs `alloc-mimalloc` vs `alloc-jemalloc` vs
  `alloc-snmalloc` where supported;
- hash-map/interner regression budget: AHash and thread-local interning are
  already present, but need fresh W0/W2 SQL, RQL, and RSS measurements;
- build-profile consistency: reconcile `docs/performance.md` with `Cargo.toml`
  before publishing benchmark-profile guidance;
- PGO script scope: keep current `scripts/perf/pgo.sh`, but benchmark against
  the fresh W0 corpus instead of stale branch artifacts.

## Test Requirements For Any Future Port

| Candidate | Minimum correctness tests | Minimum perf proof |
|---|---|---|
| Simple aggregate runtime | `redlinedb-sql` aggregate tests plus SQLite oracle cases for NULL, DISTINCT, HAVING, ORDER BY, overflow, empty input, and mixed affinities | W0-ranked aggregate cases before/after, plus `scripts/perf/quick.sh` |
| Top-K tiny buffer | Unit tests against `TopKHeap`, SQL tests for ASC/DESC, NULLS FIRST/LAST, ties, aliases, LIMIT 0/1/64/65, OFFSET | W0-ranked ORDER BY LIMIT cases before/after |
| Predicate/subquery caches | SQL tests for correlated vs uncorrelated IN, EXISTS, NOT EXISTS, scalar subquery first-row semantics, schema/stat invalidation | W0-ranked subquery and predicate cases before/after |

## Completion Notes

- No source ports were made in this W1 pass.
- No divergent branch was merged.
- `origin/perf/parity-gap-closure` is treated as subsumed by the Phase 5 squash
  merge plus later `main` changes.
- `preserve/redlinedb-sql-cli-runtime-20260524` remains the only branch with
  unique high-value runtime ideas not already visible on `main`.
