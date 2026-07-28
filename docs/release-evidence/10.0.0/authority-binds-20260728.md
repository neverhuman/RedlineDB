# Authority binds — 2026-07-28 (rows 8–11)

Committed lifecycle evidence for the release-identity rows added on the
`claude/bind-batch-3more-20260728` line, per the release policy's requirement
that lifecycle evidence lives under `docs/release-evidence/10.0.0/`.

Every row was verified against the authenticated forge immediately before it
was written: `refs/heads/main` and the named immutable tag were both read with
`git ls-remote` and both resolve to the recorded release commit. `release_tree`
is `HEAD^{tree}` of that commit; `release_checksum_sha256` is the SHA-256 of
`git archive --format=tar <commit>`, the same derivation `splitctl` applies in
its exactness check.

| member | immutable tag | release commit | release tree | archive sha256 |
|---|---|---|---|---|
| jain-report | jain-report-v10.0.0-split.0 | bd9824f77ec7e343500d964fb1b0b62c41b21c0d | 3766efc54c67efee144b95d23378e370c3edfec0 | 06ee0029a8f0720947492be46a36449e60a4085da5b12e3316c9315c76afdf33 |
| jain-core | jain-core-v10.0.0-split.1 | 93c0b3fe6d3ebeb104ee348bfe8006418ec26724 | b320033003a64f9a0b5ed3ecae773190983f717e | c72e19695490510b43c60b35fc53a521c82bb92c272365e74d39abeacf405781 |
| jain-ops | jain-ops-v10.0.0-split.0 | a9d060743ff30c0dfd80ca2d672d001efa85c335 | 9462cf19422977cb55c7d873b7a327c9b9988945 | 3f1753493dac50ec4715a32b30858de0fb9d3bed6066e59be448fed7ba0f01e3 |
| jain-jnoccio | jain-jnoccio-v10.0.0-split.0 | c1b25ec087b602385ab4e3ac61089903ae56643f | ff66f4c071ab2721e99763ecafb15bbc7b50867f | 75a84ff466d941242d51b585e05e58ef44cf5e0fefae81aa1fb346b04fd73461 |

Reviewed-route provenance, from the coordination board. Report, ops and
jnoccio completed tonight's full lifecycle with distinct identities as listed;
jain-core's route predates this record's author's visibility, so its row rests
on the pre-existing sealed receipt and forge tag identity rather than asserting
route provenance it cannot witness. The machine-readable twin of this record is
`authority-binds-20260728.json`, conforming to
`schemas/authority-bind-evidence.schema.json`:

- jain-report: authored Claude; approved Curie-95; sealed under ReleaseOps
  claim 396; merged/tagged Hopper-105.
- jain-core: pre-existing sealed receipt `cbc2cf54…` (score 88) at the exact
  release commit; tag already on the forge — bind is record-keeping only.
- jain-ops: approved Newton-102/Curie-116 line; sealed green attempt
  `af08062f…`; merged/tagged Gauss-125.
- jain-jnoccio: approved Feynman-120; sealed green under Riemann-123 claim 431;
  merged/tagged Euler-129.

Cross-validations recorded today between independently derived digests and the
computed values above: jain-report's tree matches Curie-95's approval record;
earlier rows matched ReleaseOps' tag receipt (jain-llm) and artifact source
SHA-256 (jain-jailgun). The derivation method agrees with tags as they were cut.
