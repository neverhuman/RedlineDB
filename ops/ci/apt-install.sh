#!/usr/bin/env bash
# Serialize apt installs so concurrent self-hosted runners on one host
# do not lose the dpkg/lists lock.
set -euo pipefail

if [ "$#" -lt 1 ]; then
  echo "usage: apt-install.sh <package>..." >&2
  exit 64
fi

need=()
for pkg in "$@"; do
  if ! dpkg-query -W -f='${Status}' "$pkg" 2>/dev/null | grep -q "install ok installed"; then
    need+=("$pkg")
  fi
done
if [ "${#need[@]}" -eq 0 ]; then
  exit 0
fi

lock=/var/lock/redlinedb-apt.lock
sudo mkdir -p /var/lock
sudo touch "$lock"
# Wait up to ~2 minutes for another runner's apt to finish.
sudo flock -w 120 "$lock" bash -s -- "${need[@]}" <<'EOS'
set -euo pipefail
apt-get update -qq
apt-get install -y --no-install-recommends "$@"
EOS
