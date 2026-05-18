# RedlineDB Agent Router

Mission: keep invariants local, edit the smallest lawful surface, and preserve raw evidence.

Start here:
- `agent/owner-map.json`
- `agent/test-map.json`
- `agent/proof-lanes.toml`
- `agent/generated-zones.toml`
- `agent/unsafe-ledger.toml`

Rules:
- Prefer package-scoped edits over workspace-wide edits.
- Never hand-edit paths listed in `agent/generated-zones.toml`.
- Keep active source files under 2,000 LOC; split or archive anything larger.
- Do not compress away exit codes, failing test names, panic text, spans, advisory IDs, seeds, raw-log paths, or raw-log hashes.
- Treat `just fast` as the default proof lane, then widen only when the edit crosses contract, security, or concurrency boundaries.

