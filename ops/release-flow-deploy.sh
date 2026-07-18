#!/bin/sh
set -eu

exec /usr/local/libexec/jain/splitctl release-flow \
  --manifest /home/ubuntu/jain-split/jain-split-ops/repos.manifest.toml \
  --evidence-root /home/ubuntu/jain-split/jain-split-ops/docs/release-evidence/8.0.1 \
  "$@"
