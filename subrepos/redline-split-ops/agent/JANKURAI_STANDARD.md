# Redline split control-plane standard

This independent control plane uses pinned Jankurai 1.6.11 and the protected
check `redline-split-ops/required`. `just required`, `just security`, and
`just score` are merge evidence. `just family-ci` and `just cutover-verify` are
additional cutover gates and may correctly remain red while reviewed child
heads, immutable tags, or fresh consumer evidence are unavailable.

## Ownership boundaries

Rust implementation and tests live below `tools/redline-proof/`; shell files
only launch the Rust binary or fixed external scanners. The canonical manifest
and authoritative lock remain at repository root. The compatibility lock is at
the physical family-container root `../redline.lock.toml`; it may be absent only
for the explicitly ineligible candidate state documented in `docs/testing.md`.

## Proof lanes and repair receipts

Every family or cutover failure identifies the repository or evidence field to
repair. Re-run the exact command from `docs/testing.md`; never edit eligibility,
checksums, commits, or tag metadata by hand.
