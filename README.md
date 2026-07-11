# Redline split operations

`redline-split-ops` owns the nested Redline family manifest, lock verification,
clone/update delegation, bounded family CI, and diagnostics. The four child
repositories remain independent Git repositories and are never included in an
umbrella Cargo workspace.

```text
redline-split-ops/              # this repository
redline-split/repos.manifest.toml  # compatibility mirror for the container
redline-split-ops/redline.lock.toml # immutable child pins and proof hashes
redline-split/{redline,redline-core,redline-testing,redline-web}/
```

The commands accept `REDLINE_SPLIT_ROOT` for copied or relocated checkouts:

```bash
REDLINE_SPLIT_ROOT="$PWD" ./redlinectl validate
./redlinectl clone --dry-run
./redlinectl family-ci
```

Jain delegates here through its nested-family commands. Redline remains a
separate forge family; these commands are the drill-down boundary.
