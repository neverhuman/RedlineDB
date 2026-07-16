#!/usr/bin/env bash
# Shared helpers and tool pins for every redline-web CI lane. GitHub Actions and
# local runs both source this module, so the commands are identical (ci-local
# parity). Every ops/ci/<lane>.sh sources this file via common.sh.
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WEB_DIR="${ROOT_DIR}/apps/web"
API_DIR="${ROOT_DIR}/apps/api"
ARTIFACT_DIR="${ROOT_DIR}/target/jankurai"

# When set to 1, missing tools are a hard failure instead of a skip. CI sets
# this on the runners that have the full toolchain installed.
STRICT_TOOLS="${REDLINE_STRICT_TOOLS:-0}"

# Governed auditor: caller environment and PATH never select release evidence.
readonly JAIN_GOVERNED_JANKURAI_BIN="/home/ubuntu/.jeryu/bin/jankurai"
readonly JAIN_GOVERNED_JANKURAI_VERSION="jankurai 1.6.11"
readonly JAIN_GOVERNED_JANKURAI_SHA256="fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e"

# Tool version pins (documented for ci-doctor / supply-chain parity).
NODE_PIN="${REDLINE_NODE_PIN:-22}"
RUST_PIN="${REDLINE_RUST_PIN:-stable}"

log() {
  printf '[redline-ci] %s\n' "$*"
}

warn() {
  printf '[redline-ci][warn] %s\n' "$*" >&2
}

fail() {
  printf '[redline-ci][error] %s\n' "$*" >&2
  exit 1
}

has() {
  command -v "$1" >/dev/null 2>&1
}

missing_tool() {
  local tool="$1"
  local reason="${2:-required for this check}"
  if [[ "$STRICT_TOOLS" == "1" ]]; then
    fail "missing tool: ${tool} (${reason}); install it or set REDLINE_STRICT_TOOLS=0 for bootstrap"
  fi
  warn "skipping ${tool}: not installed (${reason})"
  return 0
}

run_if_has() {
  local tool="$1"
  local reason="$2"
  shift 2
  if ! has "$tool"; then
    missing_tool "$tool" "$reason"
    return 0
  fi
  "$@"
}

repo_has() {
  [[ -e "${ROOT_DIR}/$1" ]]
}

cargo_workspace_ready() {
  repo_has Cargo.toml || return 1
  has cargo || return 1
  (cd "$ROOT_DIR" && cargo metadata --no-deps --format-version 1 >/dev/null 2>&1)
}

jain_sha256() {
  sha256sum -- "${1:?file is required}" | awk '{print $1}'
}

jain_locked_package_checksum() {
  local lock_file="${1:?lock file is required}"
  local wanted_name="${2:?package name is required}"
  local wanted_version="${3:?package version is required}"
  awk -v wanted_name="$wanted_name" -v wanted_version="$wanted_version" '
    function emit_match() {
      if (in_package && package_name == wanted_name && package_version == wanted_version) {
        print package_checksum
      }
    }
    $0 == "[[package]]" {
      emit_match()
      in_package = 1
      package_name = ""
      package_version = ""
      package_checksum = ""
      next
    }
    in_package && /^name = "/ {
      package_name = $0
      sub(/^name = "/, "", package_name)
      sub(/"$/, "", package_name)
      next
    }
    in_package && /^version = "/ {
      package_version = $0
      sub(/^version = "/, "", package_version)
      sub(/"$/, "", package_version)
      next
    }
    in_package && /^checksum = "/ {
      package_checksum = $0
      sub(/^checksum = "/, "", package_checksum)
      sub(/"$/, "", package_checksum)
      next
    }
    END { emit_match() }
  ' "$lock_file"
}

jain_seed_locked_cargo_archives() {
  local lock_file="${1:?lock file is required}"
  local cache_parent="${2:?cache parent is required}"
  local fixed_cache="${3:?fixed cache is required}"
  local cargo_home="${4:?Cargo home is required}"
  shift 4

  (( "$#" >= 2 && "$#" % 2 == 0 )) || {
    printf 'locked crate seed requires package/version pairs\n' >&2
    return 1
  }
  [[ -f "$lock_file" && ! -L "$lock_file" \
    && "$(realpath -e -- "$lock_file")" == "$lock_file" ]] || {
    printf 'Cargo.lock must be a physical regular non-symlink: %s\n' "$lock_file" >&2
    return 1
  }
  [[ -d "$cache_parent" && ! -L "$cache_parent" \
    && "$(realpath -e -- "$cache_parent")" == "$cache_parent" ]] || {
    printf 'host Cargo cache parent must be a physical non-symlink: %s\n' "$cache_parent" >&2
    return 1
  }
  [[ -d "$fixed_cache" && ! -L "$fixed_cache" \
    && "$(realpath -e -- "$fixed_cache")" == "$fixed_cache" \
    && "$(dirname -- "$fixed_cache")" == "$cache_parent" ]] || {
    printf 'fixed host Cargo cache must be one physical direct child: %s\n' "$fixed_cache" >&2
    return 1
  }
  [[ -d "$cargo_home" && ! -L "$cargo_home" \
    && "$(realpath -e -- "$cargo_home")" == "$cargo_home" ]] || {
    printf 'isolated Cargo home must be a physical non-symlink: %s\n' "$cargo_home" >&2
    return 1
  }

  local nullglob_was_set=0
  local cache_root source_archive archive_name package version checksum actual
  local destination_root destination_archive temporary_archive
  local -a cache_roots=() source_roots=() checksums=()
  shopt -q nullglob && nullglob_was_set=1
  shopt -s nullglob
  cache_roots=("$cache_parent"/index.crates.io-*)
  (( nullglob_was_set == 1 )) || shopt -u nullglob
  ((${#cache_roots[@]} > 0)) || {
    printf 'host Cargo cache contains no crates.io cache roots: %s\n' "$cache_parent" >&2
    return 1
  }
  for cache_root in "${cache_roots[@]}"; do
    [[ -d "$cache_root" && ! -L "$cache_root" \
      && "$(realpath -e -- "$cache_root")" == "$cache_root" ]] || {
      printf 'crates.io cache root must be a physical non-symlink: %s\n' "$cache_root" >&2
      return 1
    }
  done

  mkdir -p -- "$cargo_home/registry"
  [[ -d "$cargo_home/registry" && ! -L "$cargo_home/registry" ]] || {
    printf 'Cargo registry destination must be a physical directory\n' >&2
    return 1
  }
  mkdir -p -- "$cargo_home/registry/cache"
  [[ -d "$cargo_home/registry/cache" && ! -L "$cargo_home/registry/cache" ]] || {
    printf 'Cargo cache destination must be a physical directory\n' >&2
    return 1
  }
  destination_root="$cargo_home/registry/cache/$(basename -- "$fixed_cache")"
  mkdir -p -- "$destination_root"
  [[ -d "$destination_root" && ! -L "$destination_root" \
    && "$(realpath -e -- "$destination_root")" == "$destination_root" ]] || {
    printf 'Cargo archive destination must be a physical non-symlink: %s\n' \
      "$destination_root" >&2
    return 1
  }

  while (( "$#" > 0 )); do
    package="$1"
    version="$2"
    shift 2
    [[ "$package" =~ ^[A-Za-z0-9_-]+$ && "$version" =~ ^[A-Za-z0-9.+_-]+$ ]] || {
      printf 'invalid locked package identity: %s %s\n' "$package" "$version" >&2
      return 1
    }
    archive_name="$package-$version.crate"
    mapfile -t checksums < <(jain_locked_package_checksum "$lock_file" "$package" "$version")
    ((${#checksums[@]} == 1)) || {
      printf 'Cargo.lock must contain exactly one checksum for %s %s\n' \
        "$package" "$version" >&2
      return 1
    }
    checksum="${checksums[0]}"
    [[ "$checksum" =~ ^[0-9a-f]{64}$ ]] || {
      printf 'Cargo.lock checksum is invalid for %s %s\n' "$package" "$version" >&2
      return 1
    }

    source_roots=()
    for cache_root in "${cache_roots[@]}"; do
      [[ -e "$cache_root/$archive_name" || -L "$cache_root/$archive_name" ]] \
        && source_roots+=("$cache_root")
    done
    ((${#source_roots[@]} > 0)) || {
      printf 'locked crate archive is missing: %s\n' "$archive_name" >&2
      return 1
    }
    ((${#source_roots[@]} == 1)) || {
      printf 'locked crate archive has ambiguous source cache roots: %s\n' \
        "$archive_name" >&2
      return 1
    }
    [[ "${source_roots[0]}" == "$fixed_cache" ]] || {
      printf 'locked crate archive is outside the fixed cache: %s\n' "$archive_name" >&2
      return 1
    }
    source_archive="$fixed_cache/$archive_name"
    [[ -f "$source_archive" && ! -L "$source_archive" \
      && "$(realpath -e -- "$source_archive")" == "$source_archive" ]] || {
      printf 'locked crate archive must be a regular non-symlink: %s\n' \
        "$source_archive" >&2
      return 1
    }
    actual="$(jain_sha256 "$source_archive")"
    [[ "$actual" == "$checksum" ]] || {
      printf 'locked crate archive digest does not match Cargo.lock: %s\n' \
        "$archive_name" >&2
      return 1
    }

    destination_archive="$destination_root/$archive_name"
    if [[ -e "$destination_archive" || -L "$destination_archive" ]]; then
      [[ -f "$destination_archive" && ! -L "$destination_archive" \
        && "$(realpath -e -- "$destination_archive")" == "$destination_archive" \
        && "$(jain_sha256 "$destination_archive")" == "$checksum" ]] || {
        printf 'existing Cargo archive destination is not the locked artifact: %s\n' \
          "$destination_archive" >&2
        return 1
      }
      log "security: verified locked Cargo archive $archive_name sha256=$checksum"
      continue
    fi

    temporary_archive="$destination_archive.partial.$$"
    cp -- "$source_archive" "$temporary_archive"
    if [[ -L "$temporary_archive" \
      || "$(jain_sha256 "$temporary_archive")" != "$checksum" ]]; then
      rm -f -- "$temporary_archive"
      printf 'copied Cargo archive failed locked digest verification: %s\n' \
        "$archive_name" >&2
      return 1
    fi
    chmod 0644 "$temporary_archive"
    mv -- "$temporary_archive" "$destination_archive"
    [[ "$(jain_sha256 "$destination_archive")" == "$checksum" ]] || {
      printf 'seeded Cargo archive failed final verification: %s\n' "$archive_name" >&2
      return 1
    }
    log "security: seeded locked Cargo archive $archive_name sha256=$checksum"
  done
}

jain_verify_exact_executable() {
  local label="${1:?label is required}" path="${2:?path is required}"
  local expected_digest="${3:?digest is required}" resolved
  [[ -f "$path" && -x "$path" && ! -L "$path" ]] || {
    printf '%s must be an executable regular non-symlink: %s\n' "$label" "$path" >&2
    return 1
  }
  resolved="$(realpath -e -- "$path")" || return 1
  [[ "$resolved" == "$path" ]] || {
    printf '%s resolved outside its exact path: %s\n' "$label" "$resolved" >&2
    return 1
  }
  [[ "$(jain_sha256 "$path")" == "$expected_digest" ]] || {
    printf '%s digest mismatch: %s\n' "$label" "$path" >&2
    return 1
  }
}

jain_verify_governed_jankurai() {
  local path="${1:?path is required}" expected_version="${2:?version is required}"
  local expected_digest="${3:?digest is required}" actual
  jain_verify_exact_executable governed-Jankurai "$path" "$expected_digest" || return 1
  actual="$("$path" --version 2>/dev/null)" || return 1
  [[ "$actual" == "$expected_version" ]] || {
    printf 'governed Jankurai version mismatch: %s\n' "${actual:-missing}" >&2
    return 1
  }
}

jankurai_bin() {
  jain_verify_governed_jankurai \
    "$JAIN_GOVERNED_JANKURAI_BIN" \
    "$JAIN_GOVERNED_JANKURAI_VERSION" \
    "$JAIN_GOVERNED_JANKURAI_SHA256" || return 1
  printf '%s' "$JAIN_GOVERNED_JANKURAI_BIN"
}

ensure_artifacts() {
  mkdir -p "$ARTIFACT_DIR"
}

json_array() {
  if [[ "$#" -eq 0 ]]; then
    printf '[]'
    return
  fi
  printf '%s\n' "$@" | jq -R . | jq -s .
}
