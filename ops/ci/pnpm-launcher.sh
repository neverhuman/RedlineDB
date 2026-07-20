#!/bin/sh
set -eu
self_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec "$self_dir/node" "$self_dir/../lib/pnpm/bin/pnpm.cjs" "$@"
