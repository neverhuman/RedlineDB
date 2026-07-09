set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

required:
  ./ops/ci/required.sh

jeryu-doctor:
  ./ops/split/jeryu-doctor.py

jeryu-ready:
  ./ops/split/jeryu-doctor.py --fix-remotes --register-family

jeryu-repos:
  ./ops/split/jeryu-local.py repo-list

jeryu-prs repo:
  ./ops/split/jeryu-local.py pr-list --repo "{{repo}}"

jeryu-pr-open repo head title:
  ./ops/split/jeryu-local.py pr-open --repo "{{repo}}" --head "{{head}}" --title "{{title}}" --draft

fast:
  ./ops/ci/fast.sh # python3 -m pytest -q ops/split/tests

check:
  ./ops/ci/check.sh

score:
  ./ops/ci/score.sh # jankurai audit . --mode ratchet --baseline target/jankurai/accepted-baseline.json --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md

security:
  ./ops/ci/security.sh # gitleaks detect; jankurai security run . --out target/jankurai/security/evidence.json; syft; cargo audit

tool-adoption:
  ./ops/ci/tool-adoption.sh

contract-drift:
  ./ops/ci/contract-drift.sh

artifact-support:
  ./ops/ci/artifact_support.sh

profile:
  printf '%s\n' "split-ops-control-plane"
