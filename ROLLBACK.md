# Rollback

Control-plane releases are rollback-safe by tag selection. To roll back, use
the previous immutable `jain-split-ops-v*-split.N` tag and rerun:

```bash
just required
just score
```

Do not rewrite member repositories or move split tags as part of rollback.

