# Hub Install Contract

The single public-API surface of this hub is the binary install URL embedded in `install.sh`.

## Contract

```
https://github.com/neverhuman/redlinedb/releases/download/v${VERSION}/redlinedb-linux-x86_64.tar.gz
```

This URL template is verified on every PR by `ops/ci/contract-drift.sh`.

## Drift check

The generated manifest `hub-install.json` is derived from `install.sh` by `ops/ci/contract-drift.sh`. If `install.sh` is updated, regenerate the manifest:

```bash
bash ops/ci/contract-drift.sh --regen
```
