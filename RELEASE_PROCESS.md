# Release Process

1. Run `just fast`, `just required`, `just security`, and `just score`.
2. Confirm `repos.manifest.toml` points every live repo at local Jeryu tags.
3. Open and merge a local Jeryu PR for the control-plane change.
4. Tag with a new immutable `jain-split-ops-v*-split.N` tag.
5. Push the tag to local Jeryu.

Do not move an existing split tag.

