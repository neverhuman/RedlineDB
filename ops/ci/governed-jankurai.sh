#!/usr/bin/env bash
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

JBIN="$(jankurai_bin)" || fail "governed Jankurai identity verification failed"
exec "$JBIN" "$@"
