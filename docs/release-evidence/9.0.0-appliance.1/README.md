# Jain 9.0.0-appliance.1 evidence root

This evidence root is governed only by
`authority/jain-9.0.0-appliance.1.production-hotfix.toml`. It is disjoint from
`9.0.0-alpha.6`; no file here is alpha.6 evidence and `formal_ga` remains
false.

`release-spec.json` deliberately represents the current fail-closed state.
Unknown identities are `null` or `pending`, never zero pins or invented
digests. Each protected release lane replaces only the fields it actually
proves. `splitctl appliance-release plan` reports the remaining blockers.

Every operation is a dry plan unless `--apply` is present. An apply requires a
fresh envelope conforming to `appliance-release-action.v1.schema.json`, exact
authority/spec/plan digests, two sorted distinct cosign signatures from the
spec-bound owner keys, and an unconsumed nonce. The reviewed operator binary is
also path- and SHA-256-bound and receives the verified request on stdin; shell
evaluation and secrets in argv are not supported.
