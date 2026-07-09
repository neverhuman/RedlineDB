#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

printf '[security:jain-split-ops] secret-pattern scan\n' >&2
mkdir -p target/jankurai/security target/security

if command -v gitleaks >/dev/null 2>&1; then
  gitleaks detect --source . --no-git --redact --exit-code 1
else
  python3 - <<'PY'
from pathlib import Path
import re

skip = {".git", "target", ".jankurai", ".pytest_cache", "__pycache__"}
patterns = [
    re.compile(r"(?i)(api|auth|bearer|github|jeryu|merge)[_-]?(token|secret)\s*=\s*['\"][^'\"\s]{16,}['\"]"),
    re.compile(r"ghp_[A-Za-z0-9_]{30,}"),
]
errors = []
for path in Path(".").rglob("*"):
    if not path.is_file() or any(part in skip for part in path.parts):
        continue
    text = path.read_text(encoding="utf-8", errors="ignore")
    for pattern in patterns:
        if pattern.search(text):
            errors.append(str(path))
            break
if errors:
    raise SystemExit("secret-like content found in: " + ", ".join(errors))
PY
fi

printf '{"schema":"jain-split-ops.security/v1","status":"pass","scans":["gitleaks detect","secret-pattern fallback"]}\n' > target/jankurai/security/evidence.json
cp target/jankurai/security/evidence.json target/security/evidence.json
printf 'security ok: jain-split-ops\n'
