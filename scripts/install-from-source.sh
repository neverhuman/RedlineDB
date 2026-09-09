#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$root"
dest=${JERYU_INSTALL_DIR:-${REDLINEDB_INSTALL_DIR:-$HOME/.local/bin}}
bin=$root/target/release/redlinedb-cli
[[ -x $bin ]] || { printf 'missing %s; run ./scripts/build-from-source.sh first\n' "$bin" >&2; exit 1; }
mkdir -p "$dest"
install -m 755 "$bin" "$dest/redlinedb"
printf 'Installed %s/redlinedb\n' "$dest"
