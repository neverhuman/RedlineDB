#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' \
  'ops/onboard.sh is disabled: repository creation and authenticated Git push have not yet been migrated to the typed Rust Jeryu transport.' >&2
printf '%s\n' \
  'Use reviewed splitctl jeryu-local commands for supported PR and protection operations.' >&2
exit 64
