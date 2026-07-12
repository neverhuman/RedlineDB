# Release process

Run `bash ops/ci/quality-gates.sh`, then generate a fresh checksummed family CI
receipt with `just family-ci`. After every declared immutable tag is present on
the exact reviewed local-Jeryu main commit and both consumers have produced
fresh checksummed evidence, run `just proof-refresh ...` and
`just cutover-verify`. Production promotion is a separately authorized action.
