# contracts — agent guide

Read the root `AGENTS.md` first. This cell holds the machine-readable API
contract.

- **Owns:** `contracts/` — the OpenAPI mirror of `CONTRACT.md`
  (`openapi/redline-web.openapi.json`).
- **Forbidden:** handwritten transport glue or product logic here; drifting the
  OpenAPI doc from `CONTRACT.md`, `apps/api/src/model.rs`, or
  `apps/web/src/api/types.ts`.
- **Proof lane:** generation / drift checks — `bash ops/ci/contract-drift.sh`
  (`just contract-drift`), part of the PR-CI gate.

Change `CONTRACT.md` first, then this OpenAPI doc and both DTO sets together.
