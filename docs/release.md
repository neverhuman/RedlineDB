# Release

Tags are immutable. Do not move an existing split tag.

The dependency order is:

1. land control-plane policy changes in `jain-split-ops`
2. land and tag provider repos
3. update downstream Cargo Git dependencies to the new local-Jeryu tags
4. refresh locks with `cargo metadata` or `cargo generate-lockfile`
5. run each changed repo's `required` and `score` lanes
6. merge through local Jeryu and push the new immutable tag

The host CI runner may rewrite local Jeryu URLs to `file://target/bare-mirrors`
only inside its temporary CI git config. That cache is not a release source.

