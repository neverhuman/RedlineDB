# `9.0.0-distributed.1` evidence root

This directory is reserved exclusively for the unrouted distributed candidate.
It must never contain or reference an alpha.6 or appliance.1 receipt identity.

The candidate is fixed at `status=candidate`, `formal_ga=false`,
`public_routed=false`, `activation_eligible=false`, rollback `7.0.6`. The
accelerated qualification is not a soak: its companion `jain.soak-status/v1`
receipt must say `status=not_run`, `duration_seconds=0`, and
`reason=unrouted_preproduction`.

Before adding evidence, run `splitctl distributed-evidence-index` against this
root and every present historical evidence root. A historical root asserted
absent must remain absent. The index rejects symlinks, hard-linked evidence,
reused file hashes, and reused `receipt_id` values.

This README is orientation, not release evidence and not a qualification,
signature, soak, GA, route, deployment, or activation claim.
