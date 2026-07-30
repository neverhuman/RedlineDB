# Native data policy

Read the root `AGENTS.md` first. This directory documents native storage
authority; executable migrations and persistence logic remain Rust-only under
`crates/kernel` and `crates/redlinedb`.

Do not add application SQL migration runners, mutable release evidence, or
alternate data truth here. Validate storage-policy changes with
`cargo test --locked -p redlinedb --test phase8` and the repository required
lane.
