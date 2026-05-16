#!/usr/bin/env bash
if command -v sccache >/dev/null 2>&1; then
  exec sccache "$@"
else
  exec "$@"
fi
