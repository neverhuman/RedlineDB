#!/usr/bin/env python3
"""Run a subset of redline-testing sqlite_parity cases by replaying them
directly against target and reference binaries.

We use this because `redline-testing run` does not accept a case-list
filter (only `report` and `list` do). Output JSONL matches the schema
emitted by `redline-testing run` so diff.py works without changes.

Reads cases from the corpus snapshot at target/perf/corpus-snapshot.json
(produced by `scripts/perf/build-case-lists.sh`).

Usage:
  scripts/perf/run_subset.py --case-list FILE --target-bin PATH
                             --sqlite-bin PATH --output PATH
                             [--repetitions N] [--warmup N]
                             [--snapshot PATH] [--tmp-root PATH]
                             [--workers N]
"""

import argparse
import concurrent.futures
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time


SQLITE_BATCH_FLAGS = ["-batch", "-bail"]
REDLINEDB_BATCH_FLAGS = ["--batch", "--bail"]


def parse_args():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--case-list", required=True)
    ap.add_argument("--target-bin", required=True)
    ap.add_argument("--sqlite-bin", required=True)
    ap.add_argument("--output", required=True)
    ap.add_argument("--repetitions", type=int, default=3)
    ap.add_argument("--warmup", type=int, default=1)
    ap.add_argument(
        "--snapshot",
        default="target/perf/corpus-snapshot.json",
        help="path to redline-testing corpus snapshot JSON",
    )
    ap.add_argument(
        "--tmp-root",
        default=None,
        help="root for per-case tempdirs (default: tempfile.mkdtemp)",
    )
    ap.add_argument(
        "--workers",
        type=int,
        default=1,
        help="parallel case workers (default 1 for low variance)",
    )
    return ap.parse_args()


def load_case_ids(path):
    ids = []
    with open(path) as f:
        for line in f:
            line = line.split("#", 1)[0].strip()
            if not line:
                continue
            if len(line) != 5 or not line.isdigit():
                print(f"warn: ignoring invalid case-id {line!r}", file=sys.stderr)
                continue
            ids.append(line)
    return ids


def load_cases(snapshot_path, ids):
    with open(snapshot_path) as f:
        cases = json.load(f)
    by_id = {f"{c['id']:05d}": c for c in cases}
    out = []
    missing = []
    for cid in ids:
        if cid in by_id:
            out.append(by_id[cid])
        else:
            missing.append(cid)
    if missing:
        print(
            f"warn: {len(missing)} case-ids not in snapshot: {missing[:5]}{'...' if len(missing) > 5 else ''}",
            file=sys.stderr,
        )
    return out


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def query_version(binary, is_sqlite):
    try:
        if is_sqlite:
            out = subprocess.run([binary, "-version"], capture_output=True, text=True, timeout=5)
            return out.stdout.strip() or out.stderr.strip()
        else:
            out = subprocess.run([binary, "--version"], capture_output=True, text=True, timeout=5)
            return out.stdout.strip() or out.stderr.strip()
    except Exception as e:
        return f"<error: {e}>"


def is_sqlite_shell(path):
    name = os.path.basename(path).lower()
    return "sqlite" in name and "redline" not in name


def make_command(binary, case, case_tmp):
    """Mirror redline-testing/src/sqlite_parity/engine.rs::run_case command building."""
    args = case.get("args") or []
    db_path = case.get("db", ":memory:")
    cmd = [binary]
    if not args:
        flags = SQLITE_BATCH_FLAGS if is_sqlite_shell(binary) else REDLINEDB_BATCH_FLAGS
        cmd.extend(flags)
        # db_path may be ":memory:" or a relative path
        if db_path == ":memory:" or not db_path:
            cmd.append(":memory:")
        else:
            cmd.append(db_path.replace("{TMP}", str(case_tmp)))
    else:
        for arg in args:
            cmd.append(arg.replace("{TMP}", str(case_tmp)))
    return cmd


def write_fixtures(case, case_tmp):
    for entry in case.get("files") or []:
        # files schema per redline-testing: list of {name, contents}
        name = entry.get("name") if isinstance(entry, dict) else entry[0]
        contents = entry.get("contents") if isinstance(entry, dict) else entry[1]
        fpath = case_tmp / name
        fpath.parent.mkdir(parents=True, exist_ok=True)
        fpath.write_text(contents.replace("{TMP}", str(case_tmp)))


def run_one(binary, case, case_tmp, stdin_text):
    cmd = make_command(binary, case, case_tmp)
    stdin_bytes = stdin_text.encode("utf-8") if stdin_text else b""
    t0 = time.perf_counter_ns()
    proc = subprocess.run(
        cmd,
        input=stdin_bytes,
        cwd=str(case_tmp),
        capture_output=True,
        timeout=60,
    )
    elapsed_ns = time.perf_counter_ns() - t0
    return elapsed_ns, proc.returncode, proc.stdout, proc.stderr


def run_case_sample(
    case,
    target_bin,
    sqlite_bin,
    tmp_root,
    target_id,
    sqlite_id,
    sample_index,
    sample_role,
    repetition_index,
):
    """Execute a single sample of one case against both engines."""
    case_tmp = pathlib.Path(tempfile.mkdtemp(prefix=f"{case['id']:05d}-", dir=str(tmp_root)))
    try:
        write_fixtures(case, case_tmp)
        stdin = (case.get("stdin") or "").replace("{TMP}", str(case_tmp))

        ref_ns, ref_rc, ref_stdout, ref_stderr = run_one(sqlite_bin, case, case_tmp, stdin)
        tgt_ns, tgt_rc, tgt_stdout, tgt_stderr = run_one(target_bin, case, case_tmp, stdin)

        ref_sha = hashlib.sha256(ref_stdout).hexdigest()
        tgt_sha = hashlib.sha256(tgt_stdout).hexdigest()
        ratio = (tgt_ns / ref_ns) if ref_ns > 0 else 0.0

        return {
            "case_id": f"{case['id']:05d}",
            "name": case.get("name", ""),
            "case_file": case.get("folder", "") + ".rs",
            "priority": case.get("priority", ""),
            "profile": case.get("profile", ""),
            "category": case.get("category", ""),
            "sample_index": sample_index,
            "repetition_index": repetition_index,
            "sample_role": sample_role,
            "status": "passed",
            "reference_engine": "sqlite3",
            "target_engine": "redlinedb",
            "reference_executable_path": sqlite_bin,
            "target_executable_path": target_bin,
            "reference_executable_sha256": sqlite_id["sha256"],
            "target_executable_sha256": target_id["sha256"],
            "reference_version": sqlite_id["version"],
            "target_version": target_id["version"],
            "reference_elapsed_ns": ref_ns,
            "target_elapsed_ns": tgt_ns,
            "latency_ratio": ratio,
            "reference_status_code": ref_rc,
            "target_status_code": tgt_rc,
            "reference_stdout_sha256": ref_sha,
            "target_stdout_sha256": tgt_sha,
        }
    finally:
        try:
            shutil.rmtree(case_tmp, ignore_errors=True)
        except Exception:
            pass


def run_case_all_samples(case, target_bin, sqlite_bin, tmp_root, target_id, sqlite_id, repetitions, warmup):
    records = []
    sample_index = 0
    # warmup samples
    for w in range(warmup):
        records.append(
            run_case_sample(
                case, target_bin, sqlite_bin, tmp_root,
                target_id, sqlite_id,
                sample_index=sample_index,
                sample_role="warmup",
                repetition_index=None,
            )
        )
        sample_index += 1
    # measured samples
    for r in range(repetitions):
        records.append(
            run_case_sample(
                case, target_bin, sqlite_bin, tmp_root,
                target_id, sqlite_id,
                sample_index=sample_index,
                sample_role=f"measured:{r + 1}",
                repetition_index=r,
            )
        )
        sample_index += 1
    return records


def main():
    args = parse_args()

    # Binaries must be absolute because each case runs with cwd=case_tmp.
    target_bin = str(pathlib.Path(args.target_bin).resolve())
    sqlite_bin = str(pathlib.Path(args.sqlite_bin).resolve())

    ids = load_case_ids(args.case_list)
    cases = load_cases(args.snapshot, ids)
    print(f"running {len(cases)} cases x ({args.warmup} warmup + {args.repetitions} measured)", file=sys.stderr)

    target_id = {
        "path": target_bin,
        "sha256": sha256_file(target_bin),
        "version": query_version(target_bin, is_sqlite=False),
    }
    sqlite_id = {
        "path": sqlite_bin,
        "sha256": sha256_file(sqlite_bin),
        "version": query_version(sqlite_bin, is_sqlite=True),
    }

    args.target_bin = target_bin
    args.sqlite_bin = sqlite_bin

    tmp_root = pathlib.Path(args.tmp_root) if args.tmp_root else pathlib.Path(tempfile.mkdtemp(prefix="redline-testing-perf-"))
    tmp_root.mkdir(parents=True, exist_ok=True)

    out_path = pathlib.Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    all_records = []
    if args.workers > 1:
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as ex:
            futures = [
                ex.submit(
                    run_case_all_samples,
                    case, args.target_bin, args.sqlite_bin, tmp_root,
                    target_id, sqlite_id, args.repetitions, args.warmup,
                )
                for case in cases
            ]
            for f in concurrent.futures.as_completed(futures):
                all_records.extend(f.result())
    else:
        for case in cases:
            all_records.extend(
                run_case_all_samples(
                    case, args.target_bin, args.sqlite_bin, tmp_root,
                    target_id, sqlite_id, args.repetitions, args.warmup,
                )
            )

    # Sort by case_id then sample_index for stable output.
    all_records.sort(key=lambda r: (r["case_id"], r["sample_index"]))
    with open(out_path, "w") as f:
        for rec in all_records:
            f.write(json.dumps(rec, separators=(",", ":")) + "\n")
    print(f"wrote {len(all_records)} samples to {out_path}", file=sys.stderr)


if __name__ == "__main__":
    main()
