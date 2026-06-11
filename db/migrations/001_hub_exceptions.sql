-- Hub exception catalog schema (documentation/tooling only — no production DB).
-- This schema mirrors the typed exception surface in crates/hub/src/exceptions.rs
-- and provides a machine-queryable form for integration testing and tooling.
--
-- Production code: the hub is shell-only; this schema is not used at runtime.

CREATE TABLE IF NOT EXISTS hub_exceptions (
    id          TEXT    NOT NULL PRIMARY KEY  CHECK (length(id) > 0),
    purpose     TEXT    NOT NULL              CHECK (length(purpose) > 0),
    reason      TEXT    NOT NULL              CHECK (length(reason) > 0),
    common_fixes JSON   NOT NULL              CHECK (json_valid(common_fixes)),
    docs_url    TEXT    NOT NULL              CHECK (docs_url LIKE 'docs/%'),
    repair_hint TEXT    NOT NULL              CHECK (
        repair_hint LIKE 'run:%' OR repair_hint LIKE 'grep%'
    )
);

-- Seed the known ERR_* codes that map to lib.sh die() invocations.
INSERT OR IGNORE INTO hub_exceptions VALUES
    ('ERR_MISSING_TOOL',
     'locate required tool on PATH',
     'a required CLI tool was not found on PATH',
     '["run: bash scripts/ci-doctor.sh","install the missing tool per docs/testing.md#prerequisites"]',
     'docs/testing.md#agent-repair-hints',
     'run: bash scripts/ci-doctor.sh'),
    ('ERR_CONTRACT_MISMATCH',
     'verify install.sh URL template is well-formed',
     'install.sh does not contain a valid releases/download URL with a version variable',
     '["check install.sh for a hardcoded version or malformed URL","run: bash ops/ci/contract-drift.sh"]',
     'docs/testing.md#agent-repair-hints',
     'run: bash ops/ci/contract-drift.sh'),
    ('ERR_POINTER_SYNC',
     'keep README/FAMILY.md/family.json in sync',
     'a family repo pointer is missing or out of sync across the three sources',
     '["update README.md, FAMILY.md, and family.json to include the missing repo pointer"]',
     'docs/testing.md#agent-repair-hints',
     'grep -r "neverhuman/redline-core" README.md FAMILY.md family.json'),
    ('ERR_ENGINE_LEAKED',
     'enforce thin-hub invariant (no engine source in hub repo)',
     'Rust source or Cargo.toml found outside crates/hub/ — engine belongs in redline-core',
     '["move engine code to the redline-core repo","remove stray .rs files from paths other than crates/hub/"]',
     'docs/architecture.md#thin-hub-invariant',
     'run: find . -name ''*.rs'' -not -path ''./crates/hub/*'' -not -path ''./target/*'''),
    ('ERR_SECRET_DETECTED',
     'scan for accidentally committed secrets',
     'gitleaks detected a secret or high-entropy string in the repository',
     '["remove the secret and rotate it immediately","run: bash ops/ci/security.sh to re-verify"]',
     'docs/testing.md#agent-repair-hints',
     'run: bash ops/ci/security.sh');
