# contracts/

Machine-readable API contracts for redline-web. `openapi/redline-web.openapi.json`
is an OpenAPI 3.1 mirror of [`../CONTRACT.md`](../CONTRACT.md), the human source
of truth.

The contract has three representations that must agree:

| Representation | File |
|---|---|
| Human source of truth | `../CONTRACT.md` |
| Machine contract (OpenAPI) | `openapi/redline-web.openapi.json` |
| Rust DTOs (serde camelCase) | `../apps/api/src/model.rs` |
| TypeScript DTOs | `../apps/web/src/api/types.ts` |

Drift between them is checked by [`../ops/ci/contract-drift.sh`](../ops/ci/contract-drift.sh)
(`just contract-drift`), which is part of the PR-CI gate. When you change an
endpoint or DTO, update `CONTRACT.md` first, then this OpenAPI doc and both DTO
sets together.
