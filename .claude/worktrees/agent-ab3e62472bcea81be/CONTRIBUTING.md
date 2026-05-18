# Contributing to RedlineDB

Thanks for the interest in RedlineDB.

## Workflow

1. Keep changes scoped to the smallest lawful surface.
2. Add or update tests for behavior changes.
3. Run the active proof lane before asking for review.
4. Prefer small, reviewable commits with a clear intent.
5. Include raw evidence for failures: exit codes, failing test names, spans, advisory IDs, seeds, and log paths.

## Proof

- Default lane: `just fast`
- Wider lanes: `just clippy`, `just medium`, `just security-local`, `just release`
- File-size gate: `./scripts/check_file_sizes.sh`

## Receipts

- Capture the exact command that failed.
- Keep the first failing output intact when reporting regressions.
- Note any skipped checks and why they were skipped.

## Notes

- The workspace is Apache-2.0 licensed.
- Avoid committing generated artifacts or local database state.
- If a change affects public APIs, update the README or other relevant docs.
