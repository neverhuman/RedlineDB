#!/usr/bin/env bash
# Jain split cutover helper.
#
# This does not perform registry surgery by default. It verifies the deploy
# binary, installs it under ~/.jain/bin, optionally refreshes a user systemd
# service, and leaves monorepo archival to an explicit operator step.
set -euo pipefail

split="${JAIN_SPLIT_ROOT:-/home/ubuntu/jain-split}"
ops_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"  # control-plane repo (jain-split-ops)
manifest="${JAIN_SPLIT_MANIFEST:-${ops_root}/repos.manifest.toml}"
deploy="${split}/jain-deploy"
bin_src="${JAIN_BIN_SRC:-${deploy}/target/release/jain}"
install_dir="${JAIN_INSTALL_DIR:-${HOME}/.jain/bin}"
unit="${JAIN_SYSTEMD_UNIT:-${HOME}/.config/systemd/user/jain.service}"
dry_run=0
write_unit=0

usage() {
  printf 'usage: %s [--dry-run] [--write-systemd-unit]\n' "$0" >&2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) dry_run=1 ;;
    --write-systemd-unit) write_unit=1 ;;
    *) usage; exit 2 ;;
  esac
  shift
done

step() { printf '\n== %s\n' "$*"; }
run() {
  printf '+ %q' "$1"
  shift
  printf ' %q' "$@"
  printf '\n'
  if [[ "$dry_run" != "1" ]]; then
    "$@"
  fi
}

step "preconditions"
[[ -x "$bin_src" ]] || { printf 'missing release binary: %s\n' "$bin_src" >&2; exit 1; }
"$bin_src" --version | grep -Eq '^jain ' || { printf 'binary does not look like Jain\n' >&2; exit 1; }
[[ -f "${deploy}/jain-split.lock.toml" ]] || { printf 'missing deploy lock\n' >&2; exit 1; }
bash "${deploy}/scripts/stage-context.sh" --plan >/dev/null

step "install binary"
mkdir -p "$install_dir"
if [[ "$dry_run" == "1" ]]; then
  printf 'would install %s -> %s/jain\n' "$bin_src" "$install_dir"
else
  install -m 0755 "$bin_src" "${install_dir}/jain"
fi

if [[ "$write_unit" == "1" ]]; then
  step "systemd unit"
  mkdir -p "$(dirname "$unit")"
  if [[ "$dry_run" == "1" ]]; then
    printf 'would write %s\n' "$unit"
  else
    cat > "$unit" <<UNIT
[Unit]
Description=Jain API from split deploy
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
Environment=HOME=${HOME}
ExecStart=${install_dir}/jain serve --bind 127.0.0.1:8787 --split-manifest ${manifest}
Restart=always
RestartSec=2

[Install]
WantedBy=default.target
UNIT
    systemctl --user daemon-reload
    systemctl --user restart "$(basename "$unit")"
  fi
fi

step "done"
"${bin_src}" --version
