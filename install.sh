#!/usr/bin/env bash
# RedlineDB installer — fetches the latest `redline` binary from this hub's
# GitHub releases (built from a pinned redline-core tag by .github/workflows/release.yml).
#
#   curl -fsSL https://raw.githubusercontent.com/neverhuman/RedlineDB/main/install.sh | bash
#
# Env:
#   REDLINE_VERSION   pin a release tag (default: latest)
#   REDLINE_PREFIX    install dir (default: $HOME/.local/bin)
set -euo pipefail

REPO="neverhuman/RedlineDB"
PREFIX="${REDLINE_PREFIX:-$HOME/.local/bin}"
VERSION="${REDLINE_VERSION:-latest}"

err() { printf 'install: %s\n' "$*" >&2; }

detect_target() {
  local os arch
  os="$(uname -s)"; arch="$(uname -m)"
  case "$os" in
    Linux)  os=linux ;;
    Darwin) os=macos ;;
    *) err "unsupported OS: $os"; return 1 ;;
  esac
  case "$arch" in
    x86_64|amd64) arch=x86_64 ;;
    aarch64|arm64) arch=aarch64 ;;
    *) err "unsupported arch: $arch"; return 1 ;;
  esac
  printf '%s-%s' "$os" "$arch"
}

main() {
  local target tag url tmp
  target="$(detect_target)" || exit 1

  if [ "$VERSION" = "latest" ]; then
    tag="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
            | grep -m1 '"tag_name"' | cut -d'"' -f4 || true)"
  else
    tag="$VERSION"
  fi

  if [ -z "${tag:-}" ]; then
    err "no published release found yet."
    err "build from source instead:"
    err "  git clone https://github.com/neverhuman/redline-core && cd redline-core && cargo build --release"
    exit 1
  fi

  url="https://github.com/$REPO/releases/download/$tag/redline-$tag-$target.tar.gz"
  err "downloading $url"
  tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
  if ! curl -fsSL "$url" -o "$tmp/redline.tar.gz"; then
    err "no prebuilt binary for $target at $tag; build from source:"
    err "  git clone https://github.com/neverhuman/redline-core && cd redline-core && cargo build --release"
    exit 1
  fi
  tar -xzf "$tmp/redline.tar.gz" -C "$tmp"
  mkdir -p "$PREFIX"
  install -m 0755 "$tmp"/redline*/redline "$PREFIX/redline" 2>/dev/null \
    || install -m 0755 "$tmp"/redline "$PREFIX/redline"
  err "installed redline $tag -> $PREFIX/redline"
  case ":$PATH:" in *":$PREFIX:"*) ;; *) err "add $PREFIX to your PATH";; esac
}

main "$@"
