# Exception Surface — RedlineDB Hub

Agent-readable error catalog for the RedlineDB release hub. Each entry follows the
jankurai typed-exception format: `purpose`, `reason`, `common_fixes`, `docs_url`,
and `repair_hint`.

---

## ERR_MISSING_TOOL

- **purpose**: run an ops/ci lane that requires an external tool
- **reason**: required tool not found on PATH (shellcheck, gitleaks, jq, gh, python3)
- **common_fixes**:
  - install the missing tool
  - run: `bash scripts/ci-doctor.sh` — prints ok/MISSING per tool
- **docs_url**: [docs/testing.md#agent-repair-hints](../testing.md#agent-repair-hints)
- **repair_hint**: run `bash scripts/ci-doctor.sh` — exits 1 if any tool is absent; shows which lane failed

---

## ERR_CONTRACT_MISMATCH

- **purpose**: verify that install.sh URL matches contracts/install-url.txt
- **reason**: install.sh download URL does not match the committed contract
- **common_fixes**:
  - restore the `${VERSION}` variable in the install.sh download URL
  - run: `bash ops/ci/contract-drift.sh` to re-derive and diff
- **docs_url**: [docs/testing.md#agent-repair-hints](../testing.md#agent-repair-hints)
- **repair_hint**: run `bash ops/ci/contract-drift.sh` — re-derives URL from install.sh and diffs against contracts/install-url.txt

---

## ERR_POINTER_SYNC

- **purpose**: keep README.md, FAMILY.md, and family.json in sync
- **reason**: one of the three family pointer files is missing a repo entry
- **common_fixes**:
  - update all three files to agree on the family repo names
  - run: `bash ops/ci/pr-ci.sh` — pointer check step shows which file is missing what
- **docs_url**: [docs/testing.md#agent-repair-hints](../testing.md#agent-repair-hints)
- **repair_hint**: run `bash ops/ci/pr-ci.sh` — look for the pointer-check step output

---

## ERR_ENGINE_LEAKED

- **purpose**: enforce the thin-hub invariant (no engine source outside crates/domain/)
- **reason**: engine code (*.rs outside crates/domain/ or Cargo.toml at root) found in the hub
- **common_fixes**:
  - remove `.rs` files and `Cargo.toml` from outside `crates/domain/`
  - all engine code belongs in the redline-core repo
- **docs_url**: [docs/architecture.md](../architecture.md)
- **repair_hint**: run `find . -name '*.rs' -not -path './crates/domain/*' -not -path './target/*'` to locate leaks

---

## ERR_SECRET_DETECTED

- **purpose**: scan the repo for committed secrets or credentials
- **reason**: gitleaks detected a secret in the git history or working tree
- **common_fixes**:
  - remove the secret from git history with `git-filter-repo`
  - rotate the credential at the provider immediately
  - force-push the cleaned history
- **docs_url**: [SECURITY.md](../../SECURITY.md)
- **repair_hint**: run `gitleaks detect --no-banner --source .` to reproduce the finding; then remove, rotate, force-push
