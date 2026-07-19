# Architecture

`jain-split-ops` is a control-plane repository. It owns the manifest, generator,
host CI runner, Jeryu onboarding helpers, and policy validation scripts for the
Jain split family.

The family member repositories live beside this repo. Their operational remote
is local Jeryu at `http://127.0.0.1:8787/git/veox/<repo>.git`. Public release
publishing is separate from development source resolution.

The materializer writes standard files into member repos: `AGENTS.md`, CI lanes,
agent policy files, lockfiles, and generated local patch examples. The validator
keeps those outputs aligned with the local-Jeryu contract.
