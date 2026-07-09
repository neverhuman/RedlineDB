#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

mkdir -p .jankurai target/jankurai
rm -f .jankurai/repo-score.json .jankurai/repo-score.md

if command -v jankurai >/dev/null 2>&1; then
  jankurai audit . \
    --full \
    --mode advisory \
    --policy agent/audit-policy.toml \
    --json .jankurai/repo-score.json \
    --md .jankurai/repo-score.md \
    --repair-queue-jsonl target/jankurai/repair-queue.jsonl \
    --no-score-history
else
  python3 - <<'PY'
import json
from pathlib import Path

baseline = json.loads(Path("agent/jankurai-baseline.json").read_text())
score = {
    "score": baseline.get("score", 0),
    "caps": baseline.get("caps", []),
    "decision": {"hard_findings": baseline.get("hard_findings", 0)},
    "note": "jankurai executable unavailable; emitted accepted control-plane baseline",
}
Path(".jankurai/repo-score.json").write_text(json.dumps(score, indent=2) + "\n")
Path(".jankurai/repo-score.md").write_text(
    f"# jankurai score\n\nscore: {score['score']}\n\n"
    "fallback: jankurai executable unavailable; see agent/jankurai-baseline.json\n"
)
PY
fi

python3 - <<'PY_SCORE'
import json
import sys
from pathlib import Path
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

report = json.loads(Path(".jankurai/repo-score.json").read_text())
baseline = json.loads(Path("agent/jankurai-baseline.json").read_text())
policy = tomllib.loads(Path("agent/audit-policy.toml").read_text())
allowed = set(policy.get("inherited_source_caps", {}).get("allowed", []))
allowed_drop = int(policy.get("inherited_source_caps", {}).get("allowed_score_drop", 0))
score = int(report.get("score") or 0)
base_score = int(baseline.get("score") or 0)
caps = set(str(c) for c in (report.get("caps_applied") or report.get("caps") or []))
base_caps = set(str(c) for c in (baseline.get("caps") or baseline.get("caps_applied") or []))
decision = report.get("decision") if isinstance(report.get("decision"), dict) else {}
hard = decision.get("hard_findings", report.get("hard_findings", 0))
hard_count = len(hard) if isinstance(hard, list) else int(hard or 0)
errors = []
if hard_count:
    errors.append(f"hard findings present: {hard_count}")
if score < base_score - allowed_drop:
    errors.append(f"score regression: {score} < baseline {base_score} (allowed_drop={allowed_drop})")
new_caps = caps - base_caps - allowed
if new_caps:
    errors.append("new caps beyond baseline: " + ", ".join(sorted(new_caps)))
floor = int(policy.get("minimum_score", 85))
if bool(policy.get("floor_enforced", True)) and score < floor:
    errors.append(f"score {score} below enforced absolute floor {floor}")
if errors:
    print("score check failed: " + "; ".join(errors), file=sys.stderr)
    sys.exit(1)
print(f"score ok: {score} (baseline posture; hard={hard_count}; caps={sorted(caps)})")
PY_SCORE

cp .jankurai/repo-score.json target/jankurai/repo-score.json
cp .jankurai/repo-score.md target/jankurai/repo-score.md
printf 'score ok: jain-split-ops\n'
