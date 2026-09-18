#!/usr/bin/env bash
# Linux qualification artifact, not a declaration that the tested ABI is supported.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
exec python3 - <<'PYTHON'
import hashlib, json, os, pathlib, resource, signal, subprocess, tempfile

root = pathlib.Path.cwd()
out = root / "target/compatibility-phase2/abi"
out.mkdir(parents=True, exist_ok=True)
reference = root / "target/sqlite-reference/3.53.1"
source = root / "crates/ffi/tests/phase2_abi_probe.c"
probe = out / "phase2_abi_probe"
cases = ["v3-zero", "v3-persistent", "bounded-guard", "zero-guard", "empty-tail",
         "embedded-nul", "error-output", "v2-tail", "type-tags"]
sha = lambda p: hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
identity = json.loads((reference / "oracle-identity.json").read_text())
for relative in ["include/sqlite3.h", "lib/libsqlite3.so"]:
    if sha(reference / relative) != identity["artifacts"][relative]:
        raise SystemExit("infrastructure: oracle artifact does not match qualified receipt")
compiler = os.environ.get("CC", "cc")
compile_command = [compiler, "-std=gnu11", "-Wall", "-Wextra", "-Werror", "-O0", "-g",
                   "-I", str(reference / "include"), str(source), "-ldl", "-o", str(probe)]
subprocess.run(compile_command, check=True, timeout=60)
receipt = {
    "parent_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
    "dirty_status": subprocess.check_output(["git", "status", "--porcelain"], text=True),
    "compiler": subprocess.check_output([compiler, "--version"], text=True),
    "compile_command": compile_command, "source_sha256": sha(source),
    "script_sha256": sha(root / "scripts/compatibility/phase2-abi-probe.sh"),
    "probe_sha256": sha(probe), "oracle_identity": identity, "platform": os.uname()._asdict() if hasattr(os.uname(), "_asdict") else list(os.uname()),
    "cases": [], "libraries": {},
}

def limits():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    resource.setrlimit(resource.RLIMIT_FSIZE, (65536, 65536))

def run_cases(engine, library):
    library = library.resolve(strict=True)
    receipt["libraries"][engine] = {"path": str(library), "sha256": sha(library)}
    results = []
    for case in cases:
        with tempfile.TemporaryDirectory(prefix="abi-" + engine + "-") as work:
            command = [str(probe), str(library), case, str(pathlib.Path(work) / "database")]
            stdout = out / (engine + "-" + case + ".stdout")
            stderr = out / (engine + "-" + case + ".stderr")
            with stdout.open("wb") as output, stderr.open("wb") as error:
                process = subprocess.Popen(command, stdout=output, stderr=error,
                                           start_new_session=True, preexec_fn=limits)
                try:
                    code = process.wait(timeout=15)
                    classification = "pass" if code == 0 else "signal" if code < 0 else "infrastructure" if code == 125 else "failure"
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                    code, classification = None, "timeout"
            text = stdout.read_text(errors="replace")
            if not text.startswith("loaded=" + str(library) + "\n"):
                classification = "infrastructure"
            result = {"engine": engine, "case": case, "command": command, "exit": code,
                      "signal": signal.Signals(-code).name if code is not None and code < 0 else None,
                      "classification": classification, "stdout": str(stdout), "stderr": str(stderr),
                      "stdout_sha256": sha(stdout), "stderr_sha256": sha(stderr)}
            results.append(result)
            receipt["cases"].append(result)
            print(engine, case, classification, code, flush=True)
    return results

def save():
    (out / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")

oracle = run_cases("sqlite", reference / "lib/libsqlite3.so")
if any(case["classification"] != "pass" for case in oracle):
    receipt["status"] = "oracle-infrastructure-failure"
    save()
    raise SystemExit(2)
# Build from this checkout: no stale release library or accidental system SQLite.
build_command = ["cargo", "build", "--locked", "-p", "redlinedb-ffi"]
receipt["build_command"] = build_command
with (out / "build.log").open("wb") as log:
    try:
        subprocess.run(build_command, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=300)
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired):
        receipt["status"] = "build-infrastructure-failure"
        save()
        raise SystemExit(2)
receipt["build_log_sha256"] = sha(out / "build.log")
redline = run_cases("redline", root / "target/debug/libredlinedb.so")
receipt["status"] = "pass" if all(case["classification"] == "pass" for case in redline) else "failed"
save()
raise SystemExit(0 if receipt["status"] == "pass" else 1)
PYTHON
