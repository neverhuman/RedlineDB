#!/usr/bin/env python3
"""Build and execute-qualify the pinned SQLite reference, never system SQLite."""
import ctypes
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import zipfile

VERSION = "3.53.1"
SOURCE_ID = "2026-05-05 10:34:17 c88b22011a54b4f6fbd149e9f8e4de77658ce58143a1af0e3785e4e6475127e9"
ARCHIVE = "sqlite-src-3530100.zip"
# Downloaded from the upstream HTTPS release archive; manifest.uuid is also
# checked against https://sqlite.org/releaselog/3_53_1.html.
ARCHIVE_SHA3 = "27cfc9264b2188fd17f811a8c03424eb65391c2ef9874cbfc860ea25f4322363"
ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
BASE = ROOT / "target/sqlite-reference"
COMMON_SQL = """SELECT sqlite_version();
SELECT json_extract('{"a":3}', '$.a'), hex(jsonb('[]'));
CREATE TABLE t(a); INSERT INTO t VALUES(1),(2),(3);
SELECT sum(a), sqrt(9), ceil(1.2), percentile_cont(a,0.5) FROM t;
SELECT count(*)>0 FROM sqlite_dbpage;
"""
COMMON_EXPECTED = "3.53.1\n3|0B\n6|3.0|2.0|2.0\n1\n"
EXTENDED_SQL = """CREATE VIRTUAL TABLE ft USING fts5(content);
INSERT INTO ft VALUES('alpha beta');
SELECT highlight(ft,0,'[',']') FROM ft WHERE ft MATCH 'alpha';
CREATE VIRTUAL TABLE rt USING rtree(id,x1,x2);
INSERT INTO rt VALUES(1,0,10); SELECT id FROM rt WHERE x1<=5 AND x2>=5;
CREATE VIRTUAL TABLE ri USING rtree_i32(id,x1,x2);
INSERT INTO ri VALUES(2,0,10); SELECT id FROM ri WHERE x1<=5 AND x2>=5;
SELECT count(*)>0 FROM dbstat;
SELECT sqrt(9), ceil(1.2), percentile_cont(a,0.5), soundex('Robert') FROM t;
UPDATE t SET a=a+10 ORDER BY a LIMIT 1;
DELETE FROM t ORDER BY a DESC LIMIT 1;
SELECT group_concat(a) FROM (SELECT a FROM t ORDER BY a);
"""
EXTENDED_EXPECTED = "[alpha] beta\n1\n2\n1\n3.0|2.0|2.0|R163\n2,3\n"
SHELL_SQL = """SELECT sum(value) FROM generate_series(1,3);
WITH t(x) AS (VALUES('a10'),('a2'))
SELECT group_concat(x, ',') FROM (SELECT x FROM t ORDER BY x COLLATE uint);
"""


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args, **kwargs):
    return subprocess.run(args, check=True, timeout=600, **kwargs)


def probe(prefix, profile):
    sql = COMMON_SQL + (EXTENDED_SQL if profile == "extended" else "")
    expected = COMMON_EXPECTED + (EXTENDED_EXPECTED if profile == "extended" else "")
    shell = str(prefix / "bin/sqlite3")
    actual = run([shell, "-batch", "-bail", ":memory:"], input=sql,
                 text=True, capture_output=True).stdout
    if actual != expected:
        raise RuntimeError(f"shell capability execution mismatch: {actual!r} != {expected!r}")
    output = run([shell, "-batch", "-bail", ":memory:"], input=SHELL_SQL,
                 text=True, capture_output=True).stdout
    if output != "6\na2,a10\n":
        raise RuntimeError(f"shell-only capability execution mismatch: {output!r}")
    library = prefix / "lib" / ("libsqlite3.dylib" if sys.platform == "darwin" else "libsqlite3.so")
    db = ctypes.c_void_p()
    lib = ctypes.CDLL(str(library))
    lib.sqlite3_sourceid.restype = ctypes.c_char_p
    if lib.sqlite3_sourceid().decode() != SOURCE_ID:
        raise RuntimeError("embedded oracle source identity mismatch")
    lib.sqlite3_open.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.c_void_p)]
    lib.sqlite3_close.argtypes = [ctypes.c_void_p]
    callback_type = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_void_p, ctypes.c_int,
                                    ctypes.POINTER(ctypes.c_char_p), ctypes.POINTER(ctypes.c_char_p))
    rows = []
    @callback_type
    def callback(_, count, values, names):
        rows.append("|".join(values[i].decode() if values[i] else "" for i in range(count)))
        return 0
    lib.sqlite3_exec.argtypes = [ctypes.c_void_p, ctypes.c_char_p, callback_type,
                                ctypes.c_void_p, ctypes.POINTER(ctypes.c_char_p)]
    if lib.sqlite3_open(b":memory:", ctypes.byref(db)):
        raise RuntimeError("embedded oracle open failed")
    try:
        error = ctypes.c_char_p()
        rc = lib.sqlite3_exec(db, sql.encode(), callback, None, ctypes.byref(error))
        if rc or "\n".join(rows) + "\n" != expected:
            raise RuntimeError(f"embedded oracle probe rc={rc}, error={error.value!r}, rows={rows!r}")
        rows.clear()
        rc = lib.sqlite3_exec(db, b"PRAGMA compile_options;", callback, None, ctypes.byref(error))
        shell_options = run([shell, ":memory:", "PRAGMA compile_options;"],
                            text=True, capture_output=True).stdout.splitlines()
        if rc or rows != shell_options:
            raise RuntimeError("CLI and embedded oracle compile options differ")
    finally:
        lib.sqlite3_close(db)
    with tempfile.TemporaryDirectory(prefix="sqlite-shell-probe-") as temp:
        source = str(Path(temp) / "source.db")
        destination = str(Path(temp) / "recovered.db")
        run([shell, "-batch", "-bail", source], text=True, capture_output=True,
            input="CREATE TABLE recovered(id INTEGER PRIMARY KEY, value TEXT);\n"
                  "INSERT INTO recovered VALUES(7, 'recovery probe');\n")
        info = run([shell, "-batch", "-bail", source], input=".dbinfo\n",
                   text=True, capture_output=True).stdout
        fields = dict(line.rsplit(":", 1) for line in info.splitlines() if ":" in line)
        if int(fields.get("database page size", "0")) != 4096 or int(fields.get("number of tables", "0")) != 1:
            raise RuntimeError(f".dbinfo execution mismatch: {info!r}")
        recovered = run([shell, "-batch", "-bail", source], input=".recover\n",
                        text=True, capture_output=True).stdout
        run([shell, "-batch", "-bail", destination], input=recovered,
            text=True, capture_output=True)
        recovered_result = run([shell, "-batch", "-bail", destination],
            input="PRAGMA integrity_check; SELECT id,value FROM recovered;\n",
            text=True, capture_output=True).stdout
        if recovered_result != "ok\n7|recovery probe\n":
            raise RuntimeError(f".recover roundtrip mismatch: {recovered_result!r}")
    return {"sql": sql, "expected": expected, "shell_only_sql": SHELL_SQL,
            "shell_dbinfo": {"page_size": 4096, "tables": 1},
            "shell_recover_roundtrip": recovered_result,
            "shell_only_expected": "6\na2,a10\n", "cli_and_library": "passed"}


def cache_current(prefix, key):
    suffix = ".dylib" if sys.platform == "darwin" else ".so"
    required = {"bin/sqlite3", "lib/libsqlite3.a", "lib/libsqlite3" + suffix,
                "include/sqlite3.h", "include/sqlite3ext.h", "compile-options.txt"}
    try:
        previous = json.loads((prefix / "oracle-identity.json").read_text())
        artifacts = previous["artifacts"]
        return previous.get("cache_key") == key and set(artifacts) == required and all(
            (prefix / p).is_file() and digest(prefix / p) == h for p, h in artifacts.items())
    except (OSError, ValueError, KeyError, TypeError):
        return False


def main():
    profile = os.environ.get("REDLINEDB_SQLITE_REFERENCE_PROFILE", "extended")
    if profile not in ("ordinary", "extended"):
        raise ValueError("REDLINEDB_SQLITE_REFERENCE_PROFILE must be ordinary or extended")
    prefix = Path(os.environ.get("REDLINEDB_SQLITE_REFERENCE_PREFIX",
                  str(BASE / (VERSION if profile == "extended" else VERSION + "-ordinary")))).resolve()
    cc = shlex.split(os.environ.get("CC", "cc"))
    compiler = run(cc + ["--version"], text=True, capture_output=True).stdout
    flags = ["-O2", "-DSQLITE_ENABLE_COLUMN_METADATA", "-DSQLITE_ENABLE_DBPAGE_VTAB"]
    options = ["--disable-tcl", "--disable-readline", "--disable-carray", "--column-metadata"]
    if profile == "extended":
        flags += ["-DSQLITE_ENABLE_PERCENTILE", "-DSQLITE_SOUNDEX"]
        options += ["--fts5", "--rtree", "--dbstat", "--update-limit"]
    identity = {"schema_version": 1, "sqlite_version": VERSION, "source_id": SOURCE_ID,
                "archive": ARCHIVE, "archive_sha3_256": ARCHIVE_SHA3, "profile": profile,
                "compiler": compiler, "compiler_command": cc, "cflags": flags,
                "configure": options, "platform": platform.platform(),
                "machine": platform.machine(), "compiler_sha256": digest(Path(shutil.which(cc[0]))),
                "script_sha256": digest(Path(__file__)),
                "wrapper_sha256": digest(HERE / "build-reference.sh"),
                "header_probe_sha256": digest(HERE / "header-probe.c")}
    key = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
    BASE.mkdir(parents=True, exist_ok=True)
    # Serialize publication and source extraction, including custom prefix users.
    with (BASE / ".build.lock").open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        receipt = prefix / "oracle-identity.json"
        if cache_current(prefix, key):
            probe(prefix, profile)
            print(prefix / "bin/sqlite3")
            return
        downloads = BASE / "downloads"
        downloads.mkdir(exist_ok=True)
        archive = downloads / ARCHIVE
        if not archive.exists() or hashlib.sha3_256(archive.read_bytes()).hexdigest() != ARCHIVE_SHA3:
            url = os.environ.get("REDLINEDB_SQLITE_REFERENCE_URL", "https://sqlite.org/2026/" + ARCHIVE)
            with urllib.request.urlopen(url, timeout=60) as response:
                payload = response.read()
            if hashlib.sha3_256(payload).hexdigest() != ARCHIVE_SHA3:
                raise RuntimeError("SQLite source archive SHA3 mismatch")
            archive.write_bytes(payload)
        with tempfile.TemporaryDirectory(prefix="qualified-", dir=BASE) as temp:
            work = Path(temp)
            with zipfile.ZipFile(archive) as z:
                z.extractall(work)
                for entry in z.infolist():
                    mode = entry.external_attr >> 16
                    if mode:
                        (work / entry.filename).chmod(mode & 0o777)
            source = work / "sqlite-src-3530100"
            if (source / "manifest.uuid").read_text().strip() != SOURCE_ID.split()[-1]:
                raise RuntimeError("SQLite source manifest identity mismatch")
            build = work / "build"
            build.mkdir()
            # Exclude ambient compiler/linker flags from the qualified build.
            env = dict(os.environ, CC=shlex.join(cc), CFLAGS=" ".join(flags),
                       CPPFLAGS="", LDFLAGS="", LIBS="")
            log = BASE / ("build-" + profile + ".log")
            with log.open("w") as out:
                run([str(source / "configure")] + options, cwd=build, env=env, stdout=out, stderr=subprocess.STDOUT)
                run(["make", "-j", os.environ.get("REDLINEDB_SQLITE_REFERENCE_JOBS", "2"),
                     "SHELL_OPT=", "sqlite3", "libsqlite3.a", "libsqlite3" + (".dylib" if sys.platform == "darwin" else ".so")],
                    cwd=build, env=env, stdout=out, stderr=subprocess.STDOUT)
            stage = work / "install"
            for directory in ("bin", "lib", "include"):
                (stage / directory).mkdir(parents=True)
            shutil.copy2(build / "sqlite3", stage / "bin/sqlite3")
            suffix = ".dylib" if sys.platform == "darwin" else ".so"
            shutil.copy2(build / ("libsqlite3" + suffix), stage / "lib" / ("libsqlite3" + suffix))
            shutil.copy2(build / "libsqlite3.a", stage / "lib/libsqlite3.a")
            shutil.copy2(build / "sqlite3.h", stage / "include/sqlite3.h")
            shutil.copy2(source / "src/sqlite3ext.h", stage / "include/sqlite3ext.h")
            # Compile an independent consumer against the generated upstream header.
            run(cc + ["-I" + str(stage / "include"), str(HERE / "header-probe.c"),
                      str(stage / "lib/libsqlite3.a"), "-lm", "-lpthread", "-ldl", "-o", str(work / "header-probe")])
            run([str(work / "header-probe"), SOURCE_ID])
            identity["probes"] = probe(stage, profile)
            identity["compile_options"] = run([str(stage / "bin/sqlite3"), ":memory:",
                "PRAGMA compile_options;"], text=True, capture_output=True).stdout.splitlines()
            (stage / "compile-options.txt").write_text("\n".join(identity["compile_options"]) + "\n")
            identity["cache_key"] = key
            identity["build_log_sha256"] = digest(log)
            identity["artifacts"] = {str(p.relative_to(stage)): digest(p)
                                     for p in sorted(stage.rglob("*")) if p.is_file()}
            (stage / "oracle-identity.json").write_text(json.dumps(identity, indent=2, sort_keys=True) + "\n")
            # Retain the previous usable reference until all qualification passes.
            prefix.parent.mkdir(parents=True, exist_ok=True)
            backup = prefix.with_name(prefix.name + ".previous")
            if backup.exists():
                raise RuntimeError(f"previous publication needs inspection: {backup}")
            if prefix.exists():
                prefix.rename(backup)
            try:
                shutil.move(str(stage), str(prefix))
            except BaseException:
                if backup.exists():
                    backup.rename(prefix)
                raise
            if backup.exists():
                shutil.rmtree(backup)
    print(prefix / "bin/sqlite3")


if __name__ == "__main__":
    main()
