#!/usr/bin/env bash
# Auto-generated from contracts/hub-install.json by ops/ci/contract-drift.sh --regen.
# Do not edit by hand — regenerate with: bash ops/ci/contract-drift.sh --regen
readonly INSTALL_URL_TEMPLATE="https://github.com/{repo}/releases/download/{tag}/redline-{tag}-{target}.tar.gz"
readonly INSTALL_URL_PATTERN="releases/download"
readonly INSTALL_VERSION_VARIABLE="\${tag}"
readonly INSTALL_VERIFIED_BY="ops/ci/contract-drift.sh"
