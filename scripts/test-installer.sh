#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/tools" "$work/package/bin"
# A local transport fixture exercises the real installer without a release.
cat > "$work/package/bin/redlinedb" <<'BIN'
#!/bin/sh
printf 'redlinedb fixture\n'
BIN
cp "$work/package/bin/redlinedb" "$work/package/bin/redlinedb-server"
chmod +x "$work/package/bin/"*
asset=redlinedb-v4.1.0-rc.1-linux-x86_64.tar.gz
tar -czf "$work/$asset" -C "$work/package" .
(cd "$work"; if command -v sha256sum >/dev/null; then sha256sum "$asset"; else shasum -a 256 "$asset"; fi) > "$work/$asset.sha256"
cat > "$work/tools/uname" <<'BIN'
#!/bin/sh
case "$1" in -s) echo "${TEST_OS:-Linux}";; -m) echo x86_64;; esac
BIN
cat > "$work/tools/curl" <<'BIN'
#!/usr/bin/env bash
set -eu
url= out=
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o) out=$2; shift 2 ;;
    https://*) url=$1; shift ;;
    *) shift ;;
  esac
done
cp "$INSTALL_FIXTURES/${url##*/}" "$out"
BIN
chmod +x "$work/tools/"*
export INSTALL_FIXTURES=$work VERSION=v4.1.0-rc.1 PREFIX="$work/prefix with spaces"
export PATH="$work/tools:$PATH"
bash "$root/install.sh"
[[ -x $PREFIX/bin/redlinedb-server && ! -e $PREFIX/bin/sqlite3 ]]
if TEST_OS=unsupported bash "$root/install.sh" > "$work/unsupported.log" 2>&1; then exit 1; fi
grep -q 'unsupported platform' "$work/unsupported.log"
printf 'corruption' >> "$work/$asset"
if bash "$root/install.sh" > "$work/checksum.log" 2>&1; then exit 1; fi
grep -q 'checksum mismatch' "$work/checksum.log"
printf 'Installer spaces, unsupported-platform and checksum rejection tests passed.\n'
