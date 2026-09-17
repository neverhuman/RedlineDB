# apps/web — agent guide

Read the root `AGENTS.md` first. This cell is the Vite + TS + React frontend.

- **Owns:** `apps/web/` — the typed API client (`src/api/`), DTO types and
  runtime decoders (`types.ts`, `decode.ts`), React components/hooks, design
  tokens (`src/tokens.css`), and the Playwright e2e smoke (`e2e/`).
- **Forbidden:** `as`-casting untyped boundary values — validate then narrow via
  `src/api/decode.ts`; drifting DTOs from `CONTRACT.md`; JavaScript product
  files (TypeScript only); hand-editing `dist/` (it is generated and embedded by
  the backend).
- **Proof lane:** rendered UX / Playwright — `npm run test` (vitest) and
  `bash ops/ci/e2e.sh` (Playwright smoke + axe accessibility against the built
  binary).

`apps/web/dist` is built by `npm run build` and embedded by the Rust binary, so
keep `types.ts` in lock-step with `apps/api/src/model.rs` and `CONTRACT.md`.
