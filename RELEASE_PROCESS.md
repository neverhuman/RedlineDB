# Release process

Run `bash ops/ci/quality-gates.sh`, then generate a fresh checksummed family CI
receipt with `just family-ci`. After every declared immutable tag is present on
the exact reviewed local-Jeryu main commit and both consumers have produced
fresh checksummed evidence, run `just proof-refresh ...` and
`just cutover-verify`. Production promotion is a separately authorized action.

For a one-revision Core successor, the protected prepare review must precede
tag creation. After it merges, run the one-time reconciler and protect its exact
receipt separately:

```bash
./redlinectl proof-refresh --reconcile-successor \
  --receipt release-evidence/8.0.0/redline-proof-successor-jain4-reconciled.json
./redlinectl successor-receipt-verify \
  release-evidence/8.0.0/redline-proof-successor-jain4-reconciled.json
```

The reconciled state is deliberately cutover-ineligible. It only proves that
the authoritative lock and compatibility mirror are byte-identical; fresh
family CI, the immutable successor tag, both consumer proofs, normal proof
refresh, and cutover verification remain mandatory.
