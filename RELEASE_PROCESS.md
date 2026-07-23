# Release process

Run `bash ops/ci/quality-gates.sh`, then generate a fresh checksummed family CI
receipt with `just family-ci`. After every declared immutable tag is present on
the exact reviewed local-forge `veox/*` main commit and both consumers have produced
fresh checksummed evidence, run `just proof-refresh ...` and
`just cutover-verify`. Production promotion is a separately authorized action.

The retained `.jain.4` successor receipts are historical audit inputs only and
cannot validate the current six-repository `.jain.5` authority. The unchanged
four-row `.jain.3` lock, absent mirror, and unadopted `veox/*` remotes remain
explicit red gates. Fresh five-product family CI, both consumer proofs, normal
proof refresh, and cutover verification remain mandatory.
