# Rollback

If any family, consumer, tag, checksum, or freshness check fails, retain the
last reviewed ineligible lock and stop cutover. Never move a tag, force-push,
or manually edit `cutover_eligible`. Jain runtime rollback targets `7.0.6` and
is exercised only by the separately governed AtomicSoul dry-run workflow.
