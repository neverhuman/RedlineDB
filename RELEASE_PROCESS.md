# Release process

Run `bash ops/ci/quality-gates.sh`, then generate a fresh checksummed family CI
receipt with `just family-ci`. After every declared immutable tag is present on
the exact reviewed local-Jeryu main commit and both consumers have produced
fresh checksummed evidence, run `just proof-refresh ...` and
`just cutover-verify`. Production promotion is a separately authorized action.

The former Jain.4 one-revision successor flow is retained as historical,
verification-only tooling:

```bash
./redlinectl proof-refresh --reconcile-successor \
  --receipt release-evidence/8.0.0/redline-proof-successor-jain4-reconciled.json
./redlinectl successor-receipt-verify \
  release-evidence/8.0.0/redline-proof-successor-jain4-reconciled.json
```

Those receipts remain deliberately cutover-ineligible and are not Jain.6
readiness requirements. With the current mirror absent,
`review-lock-verify` reports `authoritative-only-historical`; fresh family CI,
both consumer proofs, normal proof refresh, and cutover verification remain
mandatory. Only normal proof refresh may create the current mirror pair.
