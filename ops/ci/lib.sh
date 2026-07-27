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

# Resolve a governed resource across the sealed worker and the developer host.
# The sealed worker mounts its own authority under /opt/jain-ci and exports the
# advisory sources; the host has neither. Absolute developer-home positions are
# therefore invalid as pins -- they simply do not exist inside the worker, which
# is what made the first sealed attempts die on their very first step. Candidates
# are tried in order: explicit environment override, governed mount, host path.
jain_first_present_path() {
  local candidate
  for candidate in "$@"; do
    [[ -n "$candidate" ]] || continue
    if [[ -f "$candidate" ]]; then
      printf '%s' "$candidate"
      return 0
    fi
    if [[ -d "$candidate" ]] \
      && [[ -n "$(find "$candidate" -mindepth 1 -print -quit 2>/dev/null)" ]]; then
      printf '%s' "$candidate"
      return 0
    fi
  done
  return 1
}

# Governed auditor: caller environment and PATH never select release evidence.
JAIN_GOVERNED_JANKURAI_BIN="$(jain_first_present_path \
  "${JANKURAI_BIN:-}" \
  /opt/jain-ci/authority/release-bin/jankurai \
  /usr/local/libexec/jain/jankurai \
  /home/ubuntu/.jeryu/bin/jankurai || true)"
readonly JAIN_GOVERNED_JANKURAI_BIN
readonly JAIN_GOVERNED_JANKURAI_VERSION="jankurai 1.6.11"
readonly JAIN_GOVERNED_JANKURAI_SHA256="96d99e6e7d8dc9cf23df1081edd1f975231456592f81d9405385219a2c7298aa"

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

# Resolve one commit object to its tree without trusting the checkout's mutable
# HEAD. Local cargo-audit deliberately archives this exact object from a shared
# source that may have advanced (or contain unrelated untracked files).
jain_rustsec_object_tree() {
  local repository="${1:?RustSec repository is required}"
  local commit="${2:?RustSec commit is required}"
  local tree

  [[ "$commit" =~ ^[0-9a-f]{40}$ ]] || {
    printf 'RustSec commit must be a lowercase full hash\n' >&2
    return 1
  }
  [[ -d "$repository" && ! -L "$repository" \
    && "$(realpath -e -- "$repository")" == "$repository" \
    && -d "$repository/.git" && ! -L "$repository/.git" ]] || {
    printf 'RustSec repository must be a physical checkout: %s\n' \
      "$repository" >&2
    return 1
  }
  [[ "$(git -C "$repository" cat-file -t "$commit" 2>/dev/null)" == commit ]] || {
    printf 'RustSec commit object is unavailable: %s\n' "$commit" >&2
    return 1
  }
  tree="$(git -C "$repository" rev-parse "${commit}^{tree}")" || return 1
  [[ "$tree" =~ ^[0-9a-f]{40}$ ]] || {
    printf 'RustSec commit did not resolve to a full tree hash\n' >&2
    return 1
  }
  printf '%s' "$tree"
}

# Bind the two RustSec views used by cargo-audit and cargo-deny. Release mode
# consumes the root-exported commit and requires two clean standalone detached
# snapshots at that exact object. Local mode uses the product's reviewed
# commit/tree while allowing the shared audit source HEAD to advance; the
# existing cargo-deny seeder separately enforces clean upstream lineage.
jain_resolve_rustsec_authority() {
  local mode="${1:?RustSec authority mode is required}"
  local audit_repository="${2:?cargo-audit RustSec repository is required}"
  local deny_repository="${3:?cargo-deny RustSec repository is required}"
  local release_commit="${4:-}"
  local local_commit="${5:?local RustSec commit is required}"
  local local_tree="${6:?local RustSec tree is required}"
  local commit tree audit_tree deny_tree audit_real deny_real repository

  [[ "$local_commit" =~ ^[0-9a-f]{40}$ \
    && "$local_tree" =~ ^[0-9a-f]{40}$ ]] || {
    printf 'local RustSec authority must use full commit and tree hashes\n' >&2
    return 1
  }
  audit_real="$(realpath -e -- "$audit_repository" 2>/dev/null)" || {
    printf 'cargo-audit RustSec authority is unavailable\n' >&2
    return 1
  }
  deny_real="$(realpath -e -- "$deny_repository" 2>/dev/null)" || {
    printf 'cargo-deny RustSec authority is unavailable\n' >&2
    return 1
  }
  [[ "$audit_real" != "$deny_real" ]] || {
    printf 'cargo-audit and cargo-deny RustSec authorities must be distinct\n' >&2
    return 1
  }

  case "$mode" in
    release)
      [[ "$release_commit" =~ ^[0-9a-f]{40}$ ]] || {
        printf 'release RustSec commit export must be a lowercase full hash\n' >&2
        return 1
      }
      commit="$release_commit"
      audit_tree="$(jain_rustsec_object_tree "$audit_repository" "$commit")" \
        || return 1
      deny_tree="$(jain_rustsec_object_tree "$deny_repository" "$commit")" \
        || return 1
      [[ "$audit_tree" == "$deny_tree" ]] || {
        printf 'release RustSec audit and deny trees differ\n' >&2
        return 1
      }
      for repository in "$audit_repository" "$deny_repository"; do
        [[ -z "$(find "$repository" -type l -print -quit)" ]] || {
          printf 'release RustSec snapshot contains a symlink: %s\n' \
            "$repository" >&2
          return 1
        }
        if git -C "$repository" symbolic-ref -q HEAD >/dev/null 2>&1; then
          printf 'release RustSec snapshot HEAD must be detached: %s\n' \
            "$repository" >&2
          return 1
        fi
        [[ "$(git -C "$repository" rev-parse 'HEAD^{commit}')" == "$commit" \
          && -z "$(git -C "$repository" status --porcelain=v1 \
            --untracked-files=all)" ]] || {
          printf 'release RustSec snapshot HEAD or cleanliness mismatch: %s\n' \
            "$repository" >&2
          return 1
        }
      done
      tree="$audit_tree"
      ;;
    local)
      commit="$local_commit"
      audit_tree="$(jain_rustsec_object_tree "$audit_repository" "$commit")" \
        || return 1
      deny_tree="$(jain_rustsec_object_tree "$deny_repository" "$commit")" \
        || return 1
      [[ "$audit_tree" == "$local_tree" && "$deny_tree" == "$local_tree" ]] || {
        printf 'local RustSec pinned object/tree identity mismatch\n' >&2
        return 1
      }
      tree="$local_tree"
      ;;
    *)
      printf 'RustSec authority mode must be local or release\n' >&2
      return 1
      ;;
  esac

  printf '%s\t%s\n' "$commit" "$tree"
}

# The release worker exports three names for one cargo-audit snapshot plus a
# distinct cargo-deny snapshot. Validate those names before reducing them to
# the shared commit/tree binder so missing or cross-wired root authority cannot
# be hidden by first-present-path selection.
jain_resolve_release_rustsec_authority() {
  local source_repository="${1:-}"
  local pinned_repository="${2:-}"
  local advisory_repository="${3:-}"
  local deny_repository="${4:-}"
  local release_commit="${5:-}"
  local local_commit="${6:?local RustSec commit is required}"
  local local_tree="${7:?local RustSec tree is required}"

  [[ -n "$source_repository" && -n "$pinned_repository" \
    && -n "$advisory_repository" && -n "$deny_repository" \
    && "$source_repository" == "$pinned_repository" \
    && "$advisory_repository" == "$pinned_repository" ]] || {
    printf 'release RustSec snapshot exports are missing or mismatched\n' >&2
    return 1
  }
  jain_resolve_rustsec_authority \
    release "$pinned_repository" "$deny_repository" "$release_commit" \
    "$local_commit" "$local_tree"
}

# Compute and verify the closed two-file Grype v6 database inventory published
# by the root control plane. The digest binds relative path, size, and bytes.
jain_grype_db_inventory_sha256() {
  local root="${1:?Grype database root is required}"
  local relative path actual_nodes

  [[ -d "$root" && ! -L "$root" \
    && "$(realpath -e -- "$root")" == "$root" ]] || {
    printf 'Grype database root is missing or not physical: %s\n' "$root" >&2
    return 1
  }
  actual_nodes="$(find "$root" -mindepth 1 -printf '%P\n' | LC_ALL=C sort)"
  [[ "$actual_nodes" == $'6\n6/import.json\n6/vulnerability.db' \
    && -z "$(find "$root" -mindepth 1 ! -type d ! -type f -print -quit)" ]] || {
    printf 'Grype database inventory is empty, incomplete, or not closed\n' >&2
    return 1
  }
  {
    for relative in 6/import.json 6/vulnerability.db; do
      path="$root/$relative"
      [[ -f "$path" && ! -L "$path" ]] || {
        printf 'Grype database file is not physical: %s\n' "$relative" >&2
        return 1
      }
      printf '%s\t%s\t%s\n' "$relative" "$(stat -c %s -- "$path")" \
        "$(jain_sha256 "$path")"
    done
  } | sha256sum | awk '{print $1}'
}

jain_verify_grype_db_authority() {
  local root="${1:?Grype database root is required}"
  local expected="${2:?Grype database inventory digest is required}"
  local actual relative path

  [[ "$expected" =~ ^[0-9a-f]{64}$ ]] || {
    printf 'Grype database inventory digest must be a full SHA-256\n' >&2
    return 1
  }
  actual="$(jain_grype_db_inventory_sha256 "$root")" || return 1
  [[ "$actual" == "$expected" ]] || {
    printf 'Grype database inventory digest mismatch: expected %s actual %s\n' \
      "$expected" "$actual" >&2
    return 1
  }
  [[ "$(stat -c '%u:%g:%a' -- "$root")" == '0:0:555' \
    && "$(stat -c '%u:%g:%a' -- "$root/6")" == '0:0:555' ]] || {
    printf 'Grype database directories are not immutable root authority\n' >&2
    return 1
  }
  for relative in 6/import.json 6/vulnerability.db; do
    path="$root/$relative"
    [[ "$(stat -c '%u:%g:%a:%h' -- "$path")" == '0:0:444:1' ]] || {
      printf 'Grype database file metadata is not immutable root authority: %s\n' \
        "$relative" >&2
      return 1
    }
  done
}

jain_verify_grype_db_status() {
  local status_file="${1:?Grype database status is required}"
  local root="${2:?Grype database root is required}"

  [[ -f "$status_file" && ! -L "$status_file" ]] || {
    printf 'Grype database status is missing or not physical\n' >&2
    return 1
  }
  jq -e --arg root "$root" '
    select(.valid == true)
    | select(.schemaVersion | type == "string" and test("^v6\\."))
    | select(.path == ($root + "/6/vulnerability.db"))
  ' "$status_file" >/dev/null || {
    printf 'Grype database status is invalid or unbound from its authority\n' >&2
    return 1
  }
}

jain_verify_grype_result() {
  local result_file="${1:?Grype result is required}"

  [[ -f "$result_file" && ! -L "$result_file" ]] || {
    printf 'Grype result is missing or not physical\n' >&2
    return 1
  }
  jq -e '
    select(.matches | type == "array")
    | select(
        [.matches[]? | select(
          .vulnerability.severity == "High"
          or .vulnerability.severity == "Critical"
        )] | length == 0
      )
  ' "$result_file" >/dev/null || {
    printf 'Grype result is malformed or contains high/critical findings\n' >&2
    return 1
  }
}

# Compare the exact non-root package-lock multiset with the npm PURLs emitted
# from a lock-file-only Syft scan. Duplicate name/version pairs are retained.
jain_verify_npm_lock_sbom_closure() {
  local lock_file="${1:?npm lock file is required}"
  local sbom_file="${2:?npm lock SBOM is required}"
  local expected_count="${3:?expected npm package count is required}"
  local source_name="${4:?expected Syft source name is required}"
  local expected_manifest="${5:?expected PURL manifest path is required}"
  local actual_manifest="${6:?actual PURL manifest path is required}"
  local root_purl actual_count

  [[ "$expected_count" =~ ^[0-9]+$ && "$expected_count" -gt 0 ]] || {
    printf 'expected npm package count must be positive\n' >&2
    return 1
  }
  [[ -f "$lock_file" && ! -L "$lock_file" \
    && -f "$sbom_file" && ! -L "$sbom_file" ]] || {
    printf 'npm lock and SBOM inputs must be physical files\n' >&2
    return 1
  }
  if ! jq -e --arg source_name "$source_name" '
    .spdxVersion | startswith("SPDX-")
  ' "$sbom_file" >/dev/null; then
    printf 'npm lock SBOM is not valid SPDX JSON\n' >&2
    return 1
  fi
  if ! jq -e --arg source_name "$source_name" '
      .name == $source_name
      and ([.packages[] | select(
        .name == $source_name
        and .primaryPackagePurpose == "FILE"
      )] | length == 1)
    ' "$sbom_file" >/dev/null; then
    printf 'npm lock SBOM is not bound to the lock-file-only source\n' >&2
    return 1
  fi

  root_purl="$(jq -er '
    def npm_purl($name; $version):
      "pkg:npm/" + (($name | @uri) | gsub("%2F"; "/"))
      + "@" + ($version | @uri);
    .packages[""]
    | npm_purl(.name; .version)
  ' "$lock_file")" || {
    printf 'npm lock root identity is unavailable\n' >&2
    return 1
  }
  jq -e --arg root_purl "$root_purl" --argjson expected "$expected_count" '
    [.packages[]
      | [.externalRefs[]?
        | select(.referenceType == "purl")
        | .referenceLocator
        | select(startswith("pkg:npm/"))]] as $package_purls
    | [$package_purls[][]] as $purls
    | ($purls | length) == ($expected + 1)
      and ([$package_purls[] | select(length > 0)] | length) == ($expected + 1)
      and all($package_purls[] | select(length > 0); length == 1)
      and ([$purls[] | select(. == $root_purl)] | length) == 1
  ' "$sbom_file" >/dev/null || {
    printf 'npm lock SBOM does not contain exactly one root and all npm PURLs\n' >&2
    return 1
  }
  jq -er '
    def npm_purl($name; $version):
      "pkg:npm/" + (($name | @uri) | gsub("%2F"; "/"))
      + "@" + ($version | @uri);
    .packages
    | to_entries[]
    | select(.key != "")
    | . as $entry
    | ($entry.value.name // ($entry.key | sub("^.*node_modules/"; ""))) as $name
    | npm_purl($name; $entry.value.version)
  ' "$lock_file" | LC_ALL=C sort >"$expected_manifest"
  jq -er --arg root_purl "$root_purl" '
    .packages[]
    | .externalRefs[]?
    | select(.referenceType == "purl")
    | .referenceLocator
    | select(startswith("pkg:npm/") and . != $root_purl)
  ' "$sbom_file" | LC_ALL=C sort >"$actual_manifest"

  actual_count="$(wc -l <"$actual_manifest")"
  [[ "$(wc -l <"$expected_manifest")" == "$expected_count" \
    && "$actual_count" == "$expected_count" ]] || {
    printf 'npm lock/SBOM closure count mismatch: expected %s actual %s\n' \
      "$expected_count" "$actual_count" >&2
    return 1
  }
  cmp -s -- "$expected_manifest" "$actual_manifest" || {
    printf 'npm lock name/version multiset differs from Syft npm PURLs\n' >&2
    diff -u -- "$expected_manifest" "$actual_manifest" >&2 || true
    return 1
  }
}

jain_staged_cargo_registry_inventory_sha256() {
  local registry="${1:?staged Cargo registry is required}"
  local path relative
  local -a files=()

  [[ -d "$registry/cache" && ! -L "$registry/cache" \
    && -d "$registry/index" && ! -L "$registry/index" \
    && -z "$(find "$registry/cache" "$registry/index" \
      ! -type d ! -type f -print -quit)" ]] || {
    printf 'staged Cargo registry cache/index is missing or has unsafe nodes\n' >&2
    return 1
  }
  while IFS= read -r -d '' path; do
    files+=("$path")
  done < <(find "$registry/cache" "$registry/index" -type f -print0)
  [[ "${#files[@]}" -gt 0 ]] || {
    printf 'staged Cargo registry inventory is empty\n' >&2
    return 1
  }
  {
    while IFS= read -r path; do
      relative="${path#"$registry"/}"
      printf '%s\t%s\t%s\n' "$relative" "$(stat -c %s -- "$path")" \
        "$(jain_sha256 "$path")"
    done < <(printf '%s\n' "${files[@]}" | LC_ALL=C sort)
  } | sha256sum | awk '{print $1}'
}

jain_verify_staged_cargo_registry() {
  local lock_file="${1:?Cargo lock is required}"
  local registry="${2:?staged Cargo registry is required}"
  local index_manifest="${3:?selected index manifest is required}"
  local receipt="$registry/stage-receipt.json"
  local closure="$registry/lock-source-closure.json"
  local lock_sha cache_root index_root package_count actual_count
  local name version checksum archive relative path actual
  local -a cache_children=() index_children=()
  local -A staged_expected_archives=() staged_expected_index_entries=()

  [[ -d "$registry" && ! -L "$registry" \
    && "$(realpath -e -- "$registry")" == "$registry" \
    && -f "$receipt" && ! -L "$receipt" \
    && -f "$closure" && ! -L "$closure" ]] || {
    printf 'staged Cargo registry authority is missing or not physical\n' >&2
    return 1
  }
  mapfile -t cache_children < <(
    find "$registry/cache" -mindepth 1 -maxdepth 1 -type d -print
  )
  mapfile -t index_children < <(
    find "$registry/index" -mindepth 1 -maxdepth 1 -type d -print
  )
  [[ "${#cache_children[@]}" == 1 && "${#index_children[@]}" == 1 \
    && -z "$(find "$registry/cache" "$registry/index" \
      -mindepth 1 -maxdepth 1 ! -type d -print -quit)" \
    && -z "$(find "$registry/cache" "$registry/index" \
      ! -type d ! -type f -print -quit)" ]] || {
    printf 'staged Cargo registry must have one physical cache and index child\n' >&2
    return 1
  }
  cache_root="${cache_children[0]}"
  index_root="${index_children[0]}"
  [[ "$(basename -- "$cache_root")" == "$(basename -- "$index_root")" ]] || {
    printf 'staged Cargo cache and index authorities do not share an identity\n' >&2
    return 1
  }
  [[ -z "$(find "$cache_root" -mindepth 1 -type d -print -quit)" ]] || {
    printf 'staged Cargo archive cache must be a flat physical file set\n' >&2
    return 1
  }

  jq -e '
    select(.schema_version == "jain.locked-cargo-cache/v2")
    | select(.lock_count == (.lock_sha256s | length))
    | select(.lock_sha256s == (.lock_sha256s | sort | unique))
    | select(.package_count == (.packages | length))
    | select(.package_count > 0)
    | select(.packages == (.packages | sort_by([.name, .version, .checksum])))
    | select((.packages | length)
        == (.packages | unique_by([.name, .version, .checksum]) | length))
    | select(all(.packages[];
        (.name | test("^[A-Za-z0-9_-]+$"))
        and (.version | test("^[A-Za-z0-9.+_-]+$"))
        and (.checksum | test("^[0-9a-f]{64}$"))))
  ' "$receipt" >/dev/null || {
    printf 'staged Cargo registry receipt is malformed or non-canonical\n' >&2
    return 1
  }
  jq -e --slurpfile receipt "$receipt" '
    select(.schema_version == "jain.cargo-lock-source-closure/v1")
    | select($receipt | length == 1)
    | select(.lock_count == $receipt[0].lock_count)
    | select(.lock_sha256s == $receipt[0].lock_sha256s)
  ' "$closure" >/dev/null || {
    printf 'staged Cargo lock-source closure is unbound from its receipt\n' >&2
    return 1
  }
  lock_sha="$(jain_sha256 "$lock_file")"
  jq -e --arg lock_sha "$lock_sha" \
    '.lock_sha256s | index($lock_sha) != null' "$receipt" >/dev/null || {
    printf 'staged Cargo receipt does not include the product Cargo.lock\n' >&2
    return 1
  }

  while IFS=$'\t' read -r name version checksum; do
    archive="$name-$version.crate"
    [[ -z "${staged_expected_archives[$archive]+x}" ]] || {
      printf 'staged Cargo receipt contains a duplicate archive: %s\n' "$archive" >&2
      return 1
    }
    staged_expected_archives["$archive"]="$checksum"
    relative="$(jain_cargo_index_entry_path "$name")" || return 1
    staged_expected_index_entries["$relative"]=1
  done < <(jq -r '.packages[] | [.name,.version,.checksum] | @tsv' "$receipt")
  package_count="$(jq -r '.package_count' "$receipt")"
  [[ "${#staged_expected_archives[@]}" == "$package_count" ]] || {
    printf 'staged Cargo receipt package identities are not one-to-one\n' >&2
    return 1
  }
  actual_count=0
  while IFS= read -r -d '' path; do
    archive="$(basename -- "$path")"
    [[ -n "${staged_expected_archives[$archive]+x}" \
      && -f "$path" && ! -L "$path" ]] || {
      printf 'staged Cargo cache contains an unreceipted archive: %s\n' "$archive" >&2
      return 1
    }
    actual="$(jain_sha256 "$path")"
    [[ "$actual" == "${staged_expected_archives[$archive]}" ]] || {
      printf 'staged Cargo archive checksum differs from its receipt: %s\n' "$archive" >&2
      return 1
    }
    actual_count=$((actual_count + 1))
  done < <(find "$cache_root" -mindepth 1 -maxdepth 1 -type f -print0)
  [[ "$actual_count" == "$package_count" ]] || {
    printf 'staged Cargo cache file set differs from its receipt\n' >&2
    return 1
  }

  [[ -f "$index_root/config.json" && ! -L "$index_root/config.json" ]] || {
    printf 'staged Cargo index has no physical config.json\n' >&2
    return 1
  }
  actual_count=0
  while IFS= read -r -d '' path; do
    relative="${path#"$index_root/.cache/"}"
    [[ "$relative" != "$path" \
      && -n "${staged_expected_index_entries[$relative]+x}" \
      && -f "$path" && ! -L "$path" ]] || {
      printf 'staged Cargo index contains an unreceipted entry: %s\n' "$relative" >&2
      return 1
    }
    actual_count=$((actual_count + 1))
  done < <(find "$index_root/.cache" -mindepth 1 -type f -print0)
  [[ "$actual_count" == "${#staged_expected_index_entries[@]}" ]] || {
    printf 'staged Cargo index path set differs from receipt package names\n' >&2
    return 1
  }

  actual="$(jain_cargo_index_closure_manifest_sha256 \
    "$lock_file" "$index_root")" || return 1
  [[ "$actual" == "$index_manifest" ]] || {
    printf 'staged Cargo selected-index manifest mismatch: expected %s actual %s\n' \
      "$index_manifest" "$actual" >&2
    return 1
  }
}

jain_ci_dir_identity() {
  stat -Lc '%d:%i' -- "${1:?directory is required}"
}

jain_ci_dir_nlink() {
  stat -Lc '%h' -- "${1:?directory is required}"
}

jain_ci_scratch_dir_safe() {
  local path="${1:?directory is required}" policy="${2:?mode policy is required}"
  local mode
  [[ -d "$path" && ! -L "$path" \
    && "$(realpath -e -- "$path")" == "$path" \
    && "$(stat -Lc '%u' -- "$path")" -eq "$EUID" ]] || return 1
  mode="$(stat -Lc '%a' -- "$path")"
  if [[ "$policy" == private ]]; then
    [[ "$mode" == 700 ]]
  else
    (( (8#$mode & 0002) == 0 ))
  fi
}

jain_ci_scratch_chain_matches() {
  local phase="${1:-active}" child_nlink
  jain_ci_scratch_dir_safe "$JAIN_CI_SCRATCH_ROOT" shared \
    && [[ "$(jain_ci_dir_identity "$JAIN_CI_SCRATCH_ROOT")" == "$JAIN_CI_SCRATCH_ROOT_ID" ]] \
    && [[ "$(jain_ci_dir_identity "/proc/self/fd/${JAIN_CI_SCRATCH_ROOT_FD}/.")" == "$JAIN_CI_SCRATCH_ROOT_ID" ]] \
    && [[ "$(jain_ci_dir_nlink "$JAIN_CI_SCRATCH_ROOT")" == "$JAIN_CI_SCRATCH_ROOT_NLINK" ]] \
    && jain_ci_scratch_dir_safe "$JAIN_CI_SCRATCH_TARGET" shared \
    && [[ "$(jain_ci_dir_identity "$JAIN_CI_SCRATCH_TARGET")" == "$JAIN_CI_SCRATCH_TARGET_ID" ]] \
    && [[ "$(jain_ci_dir_identity "/proc/self/fd/${JAIN_CI_SCRATCH_TARGET_FD}/.")" == "$JAIN_CI_SCRATCH_TARGET_ID" ]] \
    && [[ "$(jain_ci_dir_identity "/proc/self/fd/${JAIN_CI_SCRATCH_ROOT_FD}/target")" == "$JAIN_CI_SCRATCH_TARGET_ID" ]] \
    && [[ "$(jain_ci_dir_nlink "$JAIN_CI_SCRATCH_TARGET")" == "$JAIN_CI_SCRATCH_TARGET_NLINK" ]] \
    && jain_ci_scratch_dir_safe "$JAIN_CI_SCRATCH_PARENT" private \
    && [[ "$(jain_ci_dir_identity "$JAIN_CI_SCRATCH_PARENT")" == "$JAIN_CI_SCRATCH_PARENT_ID" ]] \
    && [[ "$(jain_ci_dir_identity "/proc/self/fd/${JAIN_CI_SCRATCH_PARENT_FD}/.")" == "$JAIN_CI_SCRATCH_PARENT_ID" ]] \
    && [[ "$(jain_ci_dir_identity "/proc/self/fd/${JAIN_CI_SCRATCH_TARGET_FD}/ci-tmp")" == "$JAIN_CI_SCRATCH_PARENT_ID" ]] \
    && [[ "$(jain_ci_dir_nlink "$JAIN_CI_SCRATCH_PARENT")" == "$JAIN_CI_SCRATCH_PARENT_NLINK" ]] \
    && jain_ci_scratch_dir_safe "$JAIN_CI_SCRATCH_PATH" private \
    && [[ "$(jain_ci_dir_identity "$JAIN_CI_SCRATCH_PATH")" == "$JAIN_CI_SCRATCH_PATH_ID" ]] \
    && [[ "$(jain_ci_dir_identity "/proc/self/fd/${JAIN_CI_SCRATCH_PATH_FD}/.")" == "$JAIN_CI_SCRATCH_PATH_ID" ]] \
    && [[ "$(jain_ci_dir_identity "/proc/self/fd/${JAIN_CI_SCRATCH_PARENT_FD}/${JAIN_CI_SCRATCH_LEAF}")" == "$JAIN_CI_SCRATCH_PATH_ID" ]] \
    || return 1
  child_nlink="$(jain_ci_dir_nlink "$JAIN_CI_SCRATCH_PATH")"
  if [[ "$phase" == empty ]]; then
    [[ "$child_nlink" == "$JAIN_CI_SCRATCH_EMPTY_NLINK" ]]
  else
    (( child_nlink >= JAIN_CI_SCRATCH_EMPTY_NLINK ))
  fi
}

jain_ci_scratch_create() {
  local root="${1:?repository root is required}" label="${2:?scratch label is required}"
  local created parent_nlink_before
  [[ "$label" =~ ^[A-Za-z0-9._-]+$ ]] || return 1
  JAIN_CI_SCRATCH_ROOT="$root"
  jain_ci_scratch_dir_safe "$JAIN_CI_SCRATCH_ROOT" shared || return 1
  JAIN_CI_SCRATCH_ROOT_ID="$(jain_ci_dir_identity "$JAIN_CI_SCRATCH_ROOT")"
  exec {JAIN_CI_SCRATCH_ROOT_FD}<"$JAIN_CI_SCRATCH_ROOT" || return 1
  [[ "$(jain_ci_dir_identity "/proc/self/fd/${JAIN_CI_SCRATCH_ROOT_FD}/.")" == "$JAIN_CI_SCRATCH_ROOT_ID" ]] \
    || return 1

  JAIN_CI_SCRATCH_TARGET="${JAIN_CI_SCRATCH_ROOT}/target"
  if [[ ! -e "$JAIN_CI_SCRATCH_TARGET" && ! -L "$JAIN_CI_SCRATCH_TARGET" ]]; then
    mkdir -m 0750 -- "/proc/self/fd/${JAIN_CI_SCRATCH_ROOT_FD}/target" || return 1
  fi
  jain_ci_scratch_dir_safe "$JAIN_CI_SCRATCH_TARGET" shared || return 1
  JAIN_CI_SCRATCH_TARGET_ID="$(jain_ci_dir_identity "$JAIN_CI_SCRATCH_TARGET")"
  exec {JAIN_CI_SCRATCH_TARGET_FD}<"/proc/self/fd/${JAIN_CI_SCRATCH_ROOT_FD}/target" \
    || return 1
  [[ "$(jain_ci_dir_identity "/proc/self/fd/${JAIN_CI_SCRATCH_TARGET_FD}/.")" == "$JAIN_CI_SCRATCH_TARGET_ID" ]] \
    || return 1

  JAIN_CI_SCRATCH_PARENT="${JAIN_CI_SCRATCH_TARGET}/ci-tmp"
  if [[ ! -e "$JAIN_CI_SCRATCH_PARENT" && ! -L "$JAIN_CI_SCRATCH_PARENT" ]]; then
    mkdir -m 0700 -- "/proc/self/fd/${JAIN_CI_SCRATCH_TARGET_FD}/ci-tmp" || return 1
  fi
  jain_ci_scratch_dir_safe "$JAIN_CI_SCRATCH_PARENT" private || return 1
  JAIN_CI_SCRATCH_PARENT_ID="$(jain_ci_dir_identity "$JAIN_CI_SCRATCH_PARENT")"
  exec {JAIN_CI_SCRATCH_PARENT_FD}<"/proc/self/fd/${JAIN_CI_SCRATCH_TARGET_FD}/ci-tmp" \
    || return 1
  [[ "$(jain_ci_dir_identity "/proc/self/fd/${JAIN_CI_SCRATCH_PARENT_FD}/.")" == "$JAIN_CI_SCRATCH_PARENT_ID" ]] \
    || return 1

  JAIN_CI_SCRATCH_ROOT_NLINK="$(jain_ci_dir_nlink "$JAIN_CI_SCRATCH_ROOT")"
  JAIN_CI_SCRATCH_TARGET_NLINK="$(jain_ci_dir_nlink "$JAIN_CI_SCRATCH_TARGET")"
  parent_nlink_before="$(jain_ci_dir_nlink "$JAIN_CI_SCRATCH_PARENT")"

  created="$(mktemp -d "/proc/self/fd/${JAIN_CI_SCRATCH_PARENT_FD}/${label}.XXXXXX")" \
    || return 1
  JAIN_CI_SCRATCH_LEAF="${created##*/}"
  JAIN_CI_SCRATCH_PATH="${JAIN_CI_SCRATCH_PARENT}/${JAIN_CI_SCRATCH_LEAF}"
  jain_ci_scratch_dir_safe "$JAIN_CI_SCRATCH_PATH" private || return 1
  JAIN_CI_SCRATCH_PATH_ID="$(jain_ci_dir_identity "$JAIN_CI_SCRATCH_PATH")"
  exec {JAIN_CI_SCRATCH_PATH_FD}<"/proc/self/fd/${JAIN_CI_SCRATCH_PARENT_FD}/${JAIN_CI_SCRATCH_LEAF}" \
    || return 1
  JAIN_CI_SCRATCH_PARENT_NLINK="$(jain_ci_dir_nlink "$JAIN_CI_SCRATCH_PARENT")"
  (( JAIN_CI_SCRATCH_PARENT_NLINK == parent_nlink_before + 1 )) || return 1
  JAIN_CI_SCRATCH_EMPTY_NLINK="$(jain_ci_dir_nlink "$JAIN_CI_SCRATCH_PATH")"
  [[ "$JAIN_CI_SCRATCH_EMPTY_NLINK" == 2 ]] || return 1
  jain_ci_scratch_chain_matches empty
}

jain_ci_scratch_remove() {
  jain_ci_scratch_chain_matches || {
    printf 'refusing scratch cleanup: directory custody changed\n' >&2
    return 1
  }
  find -P "/proc/self/fd/${JAIN_CI_SCRATCH_PATH_FD}/." -xdev -depth -mindepth 1 -delete \
    || return 1
  jain_ci_scratch_chain_matches empty || {
    printf 'refusing scratch retention: directory custody changed\n' >&2
    return 1
  }
  [[ -z "$(find -P "/proc/self/fd/${JAIN_CI_SCRATCH_PATH_FD}/." -mindepth 1 -print -quit)" ]]
}

jain_verify_cargo_deny_clean_log() {
  local log_file="${1:?cargo-deny log is required}"
  local expected_sha256="f1a0fca39d4280363937aabd77783990ea6480bd9ca257816de3b68fc8efa845"

  [[ -f "$log_file" && ! -L "$log_file" \
    && "$(realpath -e -- "$log_file")" == "$log_file" \
    && "$(wc -c <"$log_file")" -eq 48 \
    && "$(jain_sha256 "$log_file")" == "$expected_sha256" ]] || {
    printf 'cargo-deny log must be the exact LF-terminated passing summary\n' >&2
    return 1
  }
}

jain_locked_package_checksum() {
  local lock_file="${1:?lock file is required}"
  local wanted_name="${2:?package name is required}"
  local wanted_version="${3:?package version is required}"
  awk -v wanted_name="$wanted_name" -v wanted_version="$wanted_version" '
    function emit_match() {
      if (in_package && package_name == wanted_name && package_version == wanted_version \
          && package_source == "registry+https://github.com/rust-lang/crates.io-index") {
        print package_checksum
      }
    }
    $0 == "[[package]]" {
      emit_match()
      in_package = 1
      package_name = ""
      package_version = ""
      package_source = ""
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
    in_package && /^source = "/ {
      package_source = $0
      sub(/^source = "/, "", package_source)
      sub(/"$/, "", package_source)
      next
    }
    END { emit_match() }
  ' "$lock_file"
}

jain_locked_registry_package_records() {
  local lock_file="${1:?lock file is required}"
  local records

  [[ -f "$lock_file" && ! -L "$lock_file" \
    && "$(realpath -e -- "$lock_file")" == "$lock_file" ]] || {
    printf 'Cargo.lock must be a physical regular non-symlink: %s\n' "$lock_file" >&2
    return 1
  }
  records="$({
    awk '
      function reject(message) {
        print message > "/dev/stderr"
        invalid = 1
      }
      function emit_package( key) {
        if (!in_package || package_source !~ /^registry\+/) {
          return
        }
        if (package_source != "registry+https://github.com/rust-lang/crates.io-index") {
          reject("Cargo.lock contains an unsupported registry source: " package_source)
          return
        }
        if (package_name !~ /^[A-Za-z0-9_-]+$/ \
            || package_version !~ /^[A-Za-z0-9.+_-]+$/ \
            || length(package_checksum) != 64 \
            || package_checksum !~ /^[0-9a-f]+$/) {
          reject("Cargo.lock registry package identity/checksum is invalid: " \
            package_name " " package_version)
          return
        }
        key = package_name SUBSEP package_version
        if (seen[key]++) {
          reject("Cargo.lock contains a duplicate registry package identity: " \
            package_name " " package_version)
          return
        }
        print package_name "\t" package_version "\t" package_checksum
        emitted++
      }
      $0 == "[[package]]" {
        emit_package()
        in_package = 1
        package_name = ""
        package_version = ""
        package_source = ""
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
      in_package && /^source = "/ {
        package_source = $0
        sub(/^source = "/, "", package_source)
        sub(/"$/, "", package_source)
        next
      }
      in_package && /^checksum = "/ {
        package_checksum = $0
        sub(/^checksum = "/, "", package_checksum)
        sub(/"$/, "", package_checksum)
        next
      }
      END {
        emit_package()
        if (!emitted) {
          reject("Cargo.lock contains no registry package closure")
        }
        if (invalid) {
          exit 1
        }
      }
    ' "$lock_file"
  } | LC_ALL=C sort -t $'\t' -k1,1 -k2,2 -k3,3)" || return 1
  printf '%s\n' "$records"
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
  [[ -z "$(find "$fixed_cache" -mindepth 1 ! -type f -print -quit)" ]] || {
    printf 'fixed host Cargo cache must be a flat physical file set: %s\n' \
      "$fixed_cache" >&2
    return 1
  }
  [[ -d "$cargo_home" && ! -L "$cargo_home" \
    && "$(realpath -e -- "$cargo_home")" == "$cargo_home" ]] || {
    printf 'isolated Cargo home must be a physical non-symlink: %s\n' "$cargo_home" >&2
    return 1
  }

  local source_archive archive_name package version checksum actual
  local destination_root destination_archive temporary_archive
  local -a checksums=()

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

    source_archive="$fixed_cache/$archive_name"
    [[ -e "$source_archive" || -L "$source_archive" ]] || {
      printf 'locked crate archive is missing: %s\n' "$archive_name" >&2
      return 1
    }
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

# Seed ONLY the crates.io index entries for the crates this Cargo.lock resolves.
#
# cargo-deny needs an index to answer "is this crate version yanked?"; without one
# it emits error[index-failure] and the advisories check fails. The lane used to
# satisfy that by copying the entire developer index and pinning it by
# whole-directory digest, which fired on any unrelated crate resolution anywhere on
# the host. Scoping the copy to the lockfile closure keeps the yanked check and
# makes the seeded set a function of committed bytes.
#
# A selected index entry can change when a new version of that crate is published.
# Its reviewed content manifest is therefore pinned here, and an authority refresh
# requires an explicit reviewed pin bump. Completeness and exactness are independent
# controls: every locked registry crate must have an entry, and the isolated index
# may contain no file, directory, symlink, or special node outside that closure.
jain_seed_locked_cargo_registry_index() {
  local lock_file="${1:?lock file is required}"
  local source_index="${2:?source registry index is required}"
  local cargo_home="${3:?isolated Cargo home is required}"
  local expected_manifest="${4:?closure index manifest is required}"
  local destination name relative seeded=0 actual
  local -a names=()

  [[ "$expected_manifest" =~ ^[0-9a-f]{64}$ ]] || {
    printf 'closure index manifest must be a full SHA-256\n' >&2
    return 1
  }

  [[ -f "$lock_file" && ! -L "$lock_file" ]] || {
    printf 'locked registry index seed requires a physical lock file: %s\n' \
      "$lock_file" >&2
    return 1
  }
  [[ -d "$source_index" && ! -L "$source_index" \
    && "$(realpath -e -- "$source_index")" == "$source_index" \
    && -z "$(find "$source_index" -type l -print -quit)" ]] || {
    printf 'source crates.io index must be a physical symlink-free directory: %s\n' \
      "$source_index" >&2
    return 1
  }

  # Content custody on the SOURCE closure before anything is copied.
  actual="$(jain_cargo_index_closure_manifest_sha256 "$lock_file" "$source_index")" \
    || return 1
  [[ "$actual" == "$expected_manifest" ]] || {
    printf 'source lock-closure index manifest mismatch: expected %s actual %s\n' \
      "$expected_manifest" "$actual" >&2
    return 1
  }

  destination="$cargo_home/registry/index/$(basename -- "$source_index")"
  [[ ! -e "$destination" && ! -L "$destination" ]] || {
    printf 'isolated crates.io index destination already exists: %s\n' "$destination" >&2
    return 1
  }
  mkdir -p "$destination/.cache"
  [[ -f "$source_index/config.json" && ! -L "$source_index/config.json" ]] || {
    printf 'source crates.io index lacks a physical config.json\n' >&2
    return 1
  }
  cp -- "$source_index/config.json" "$destination/config.json"

  mapfile -t names < <(
    awk '/^name = "/ { gsub(/^name = "|"$/, ""); print }' "$lock_file" \
      | LC_ALL=C sort -u
  )
  [[ "${#names[@]}" -gt 0 ]] || {
    printf 'lock file yielded no crate names: %s\n' "$lock_file" >&2
    return 1
  }

  for name in "${names[@]}"; do
    relative="$(jain_cargo_index_entry_path "$name")" || return 1
    # Workspace members are not registry crates and have no index entry.
    [[ -f "$source_index/.cache/$relative" ]] || continue
    [[ ! -L "$source_index/.cache/$relative" ]] || {
      printf 'source index entry is a symlink: %s\n' "$relative" >&2
      return 1
    }
    mkdir -p "$destination/.cache/$(dirname -- "$relative")"
    cp -- "$source_index/.cache/$relative" "$destination/.cache/$relative"
    seeded=$((seeded + 1))
  done

  [[ "$seeded" -gt 0 ]] || {
    printf 'no locked crate resolved to a crates.io index entry\n' >&2
    return 1
  }
  # Exact physical-set and content custody on the destination immediately after
  # copying. The same verifier runs after cargo-deny to reject post-seed extras.
  jain_verify_locked_cargo_registry_index \
    "$lock_file" "$destination" "$expected_manifest" || return 1
  log "security: seeded lock-closure crates.io index entries=$seeded manifest=$expected_manifest"
}

jain_locked_cargo_index_expected_files() {
  local lock_file="${1:?lock file is required}"
  local index_root="${2:?index root is required}"
  local records name relative
  local -a names=() files=(config.json)

  records="$(jain_locked_registry_package_records "$lock_file")" || return 1
  mapfile -t names < <(
    cut -f1 <<<"$records" | LC_ALL=C sort -u
  )
  [[ "${#names[@]}" -gt 0 ]] || {
    printf 'Cargo.lock yielded no crates.io index names\n' >&2
    return 1
  }
  [[ -f "$index_root/config.json" && ! -L "$index_root/config.json" ]] || {
    printf 'crates.io index lacks a physical config.json: %s\n' "$index_root" >&2
    return 1
  }
  for name in "${names[@]}"; do
    relative="$(jain_cargo_index_entry_path "$name")" || return 1
    [[ -f "$index_root/.cache/$relative" \
      && ! -L "$index_root/.cache/$relative" ]] || {
      printf 'crates.io index lacks a physical locked entry: %s\n' "$relative" >&2
      return 1
    }
    files+=(".cache/$relative")
  done
  printf '%s\n' "${files[@]}" | LC_ALL=C sort
}

# Deterministic SHA-256 over ONLY the Cargo.lock-selected index entries: the
# sorted relative paths and bytes of those entries, plus config.json. Unrelated
# growth of the shared cache cannot move this value, while a legitimate upstream
# change to a SELECTED crate's entry does -- which must then be refreshed by an
# explicit reviewed manifest bump rather than silently changing a release
# security decision.
jain_cargo_index_closure_manifest_sha256() {
  local lock_file="${1:?lock file is required}"
  local index_root="${2:?index root is required}"
  local selected_files selected_file manifest_relative
  local -a files=()

  selected_files="$(jain_locked_cargo_index_expected_files \
    "$lock_file" "$index_root")" || return 1
  mapfile -t files <<<"$selected_files"
  {
    printf 'config.json\n'
    sha256sum -- "$index_root/config.json" | awk '{print $1}'
    for selected_file in "${files[@]}"; do
      [[ "$selected_file" == .cache/* ]] || continue
      manifest_relative="${selected_file#.cache/}"
      printf '%s\n' "$manifest_relative"
      sha256sum -- "$index_root/$selected_file" | awk '{print $1}'
    done
  } | sha256sum | awk '{print $1}'
}

# Re-verify an already-materialised isolated index against the pinned closure
# manifest. Used immediately after copying and again after cargo-deny has run, so
# a mutation during the governed decision cannot pass unnoticed.
jain_verify_locked_cargo_registry_index() {
  local lock_file="${1:?lock file is required}"
  local index_root="${2:?index root is required}"
  local expected_manifest="${3:?closure index manifest is required}"
  local actual expected_files actual_files expected_dirs actual_dirs
  local file_path directory
  local -a files=()
  local -A directory_set=()

  [[ -d "$index_root" && ! -L "$index_root" \
    && "$(realpath -e -- "$index_root")" == "$index_root" ]] || {
    printf 'isolated crates.io index is not a physical directory\n' >&2
    return 1
  }
  [[ -z "$(find "$index_root" -mindepth 1 ! -type d ! -type f -print -quit)" ]] || {
    printf 'isolated crates.io index contains a symlink or special node\n' >&2
    return 1
  }
  expected_files="$(jain_locked_cargo_index_expected_files \
    "$lock_file" "$index_root")" || return 1
  actual_files="$(find "$index_root" -mindepth 1 -type f -printf '%P\n' \
    | LC_ALL=C sort)"
  [[ "$actual_files" == "$expected_files" ]] || {
    printf 'isolated crates.io index file set differs from the lock closure\n' >&2
    return 1
  }

  mapfile -t files <<<"$expected_files"
  for file_path in "${files[@]}"; do
    directory="$(dirname -- "$file_path")"
    while [[ "$directory" != "." ]]; do
      directory_set["$directory"]=1
      directory="$(dirname -- "$directory")"
    done
  done
  expected_dirs="$(
    printf '%s\n' "${!directory_set[@]}" | LC_ALL=C sort
  )"
  actual_dirs="$(find "$index_root" -mindepth 1 -type d -printf '%P\n' \
    | LC_ALL=C sort)"
  [[ "$actual_dirs" == "$expected_dirs" ]] || {
    printf 'isolated crates.io index directory set differs from the lock closure\n' >&2
    return 1
  }

  actual="$(jain_cargo_index_closure_manifest_sha256 "$lock_file" "$index_root")" \
    || return 1
  [[ "$actual" == "$expected_manifest" ]] || {
    printf 'isolated lock-closure index manifest mismatch: expected %s actual %s\n' \
      "$expected_manifest" "$actual" >&2
    return 1
  }
}

# Cargo's index path scheme for a crate name.
jain_cargo_index_entry_path() {
  local name="${1:?crate name is required}"
  local lowered
  lowered="$(printf '%s' "$name" | tr '[:upper:]' '[:lower:]')"
  case "${#lowered}" in
    1) printf '1/%s' "$lowered" ;;
    2) printf '2/%s' "$lowered" ;;
    3) printf '3/%s/%s' "${lowered:0:1}" "$lowered" ;;
    *) printf '%s/%s/%s' "${lowered:0:2}" "${lowered:2:2}" "$lowered" ;;
  esac
}

jain_seed_locked_cargo_registry_closure() {
  local lock_file="${1:?lock file is required}"
  local cache_parent="${2:?cache parent is required}"
  local fixed_cache="${3:?fixed cache is required}"
  local cargo_home="${4:?Cargo home is required}"
  local records destination_root package version checksum
  local -a pairs=()

  records="$(jain_locked_registry_package_records "$lock_file")" || return 1
  while IFS=$'\t' read -r package version checksum; do
    [[ -n "$package" && -n "$version" && "$checksum" =~ ^[0-9a-f]{64}$ ]] || {
      printf 'locked registry closure record is malformed\n' >&2
      return 1
    }
    pairs+=("$package" "$version")
  done <<<"$records"
  ((${#pairs[@]} > 0)) || {
    printf 'locked registry closure is empty\n' >&2
    return 1
  }

  destination_root="$cargo_home/registry/cache/$(basename -- "$fixed_cache")"
  [[ ! -e "$destination_root" && ! -L "$destination_root" ]] || {
    printf 'locked registry closure destination must not preexist: %s\n' \
      "$destination_root" >&2
    return 1
  }
  jain_seed_locked_cargo_archives \
    "$lock_file" "$cache_parent" "$fixed_cache" "$cargo_home" "${pairs[@]}" \
    || return 1

  jain_verify_locked_cargo_registry_closure \
    "$lock_file" "$fixed_cache" "$cargo_home" || return 1
}

jain_verify_locked_cargo_registry_closure() {
  local lock_file="${1:?lock file is required}"
  local fixed_cache="${2:?fixed cache is required}"
  local cargo_home="${3:?Cargo home is required}"
  local records destination_root package version checksum archive
  local expected_manifest actual_manifest
  local -a expected_archives=() actual_archives=()

  records="$(jain_locked_registry_package_records "$lock_file")" || return 1
  [[ -d "$fixed_cache" && ! -L "$fixed_cache" \
    && "$(realpath -e -- "$fixed_cache")" == "$fixed_cache" \
    && -d "$cargo_home" && ! -L "$cargo_home" \
    && "$(realpath -e -- "$cargo_home")" == "$cargo_home" ]] || {
    printf 'Cargo registry closure verifier requires physical cache/home roots\n' >&2
    return 1
  }
  while IFS=$'\t' read -r package version checksum; do
    expected_archives+=("$package-$version.crate")
  done <<<"$records"

  destination_root="$cargo_home/registry/cache/$(basename -- "$fixed_cache")"
  [[ -d "$destination_root" && ! -L "$destination_root" ]] || {
    printf 'isolated Cargo archive closure is missing or nonphysical: %s\n' \
      "$destination_root" >&2
    return 1
  }
  mapfile -t actual_archives < <(
    find "$destination_root" -mindepth 1 -maxdepth 1 -printf '%f\n' | LC_ALL=C sort
  )
  mapfile -t expected_archives < <(printf '%s\n' "${expected_archives[@]}" | LC_ALL=C sort)
  [[ "${actual_archives[*]}" == "${expected_archives[*]}" ]] || {
    printf 'isolated Cargo archive destination contains missing or unlocked entries\n' >&2
    return 1
  }
  while IFS=$'\t' read -r package version checksum; do
    archive="$destination_root/$package-$version.crate"
    [[ -f "$archive" && ! -L "$archive" \
      && "$(realpath -e -- "$archive")" == "$archive" \
      && "$(jain_sha256 "$archive")" == "$checksum" ]] || {
      printf 'isolated Cargo archive closure digest mismatch: %s\n' "$archive" >&2
      return 1
    }
  done <<<"$records"
  expected_manifest="$(printf '%s\n' "$records" | sha256sum | awk '{print $1}')"
  actual_manifest="$(
    while IFS=$'\t' read -r package version checksum; do
      printf '%s\t%s\t%s\n' "$package" "$version" \
        "$(jain_sha256 "$destination_root/$package-$version.crate")"
    done <<<"$records" | sha256sum | awk '{print $1}'
  )"
  [[ "$actual_manifest" == "$expected_manifest" ]] || {
    printf 'isolated Cargo archive closure manifest mismatch\n' >&2
    return 1
  }
  log "security: verified exact Cargo.lock registry closure packages=${#expected_archives[@]} manifest=$actual_manifest"
}

jain_directory_manifest_sha256() {
  local root="${1:?directory is required}"
  (
    cd "$root"
    find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum
  ) | sha256sum | awk '{print $1}'
}

jain_seed_cargo_registry_index() {
  local source_index="${1:?source registry index is required}"
  local cargo_home="${2:?isolated Cargo home is required}"
  local expected_manifest="${3:?registry index manifest is required}"
  local destination

  [[ "$expected_manifest" =~ ^[0-9a-f]{64}$ ]] || {
    printf 'registry index manifest must be a full SHA-256\n' >&2
    return 1
  }
  [[ -d "$source_index" && ! -L "$source_index" \
    && "$(realpath -e -- "$source_index")" == "$source_index" \
    && -z "$(find "$source_index" -type l -print -quit)" \
    && "$(jain_directory_manifest_sha256 "$source_index")" == "$expected_manifest" ]] || {
    printf 'fixed crates.io registry index physical identity mismatch: %s\n' \
      "$source_index" >&2
    return 1
  }
  [[ -d "$cargo_home" && ! -L "$cargo_home" \
    && "$(realpath -e -- "$cargo_home")" == "$cargo_home" \
    && -z "$(find "$cargo_home" -mindepth 1 -print -quit)" ]] || {
    printf 'registry index destination home must be new, empty, and physical: %s\n' \
      "$cargo_home" >&2
    return 1
  }

  mkdir -p "$cargo_home/registry/index"
  destination="$cargo_home/registry/index/$(basename -- "$source_index")"
  cp -a --reflink=auto -- "$source_index" "$destination"
  [[ -d "$destination" && ! -L "$destination" \
    && -z "$(find "$destination" -type l -print -quit)" \
    && "$(jain_directory_manifest_sha256 "$destination")" == "$expected_manifest" ]] || {
    printf 'isolated crates.io registry index copy identity mismatch\n' >&2
    return 1
  }
  log "security: materialized isolated crates.io index manifest=$expected_manifest"
}

jain_verify_isolated_cargo_deny_db() {
  local repository="${1:?repository is required}"
  local expected_commit="${2:?commit is required}"
  local expected_tree="${3:?tree is required}"
  local expected_config actual_config
  local -a fetch_lines=()

  [[ "$expected_commit" =~ ^[0-9a-f]{40}$ && "$expected_tree" =~ ^[0-9a-f]{40}$ ]] || {
    printf 'cargo-deny advisory identity must use full commit and tree hashes\n' >&2
    return 1
  }
  [[ -d "$repository" && ! -L "$repository" \
    && "$(realpath -e -- "$repository")" == "$repository" \
    && -d "$repository/.git" && ! -L "$repository/.git" ]] || {
    printf 'isolated cargo-deny advisory DB must be a physical checkout: %s\n' \
      "$repository" >&2
    return 1
  }
  [[ -z "$(find "$repository" -type l -print -quit)" ]] || {
    printf 'isolated cargo-deny advisory DB contains a symlink: %s\n' "$repository" >&2
    return 1
  }
  [[ ! -e "$repository/.git/objects/info/alternates" \
    && -z "$(find "$repository/.git/hooks" -mindepth 1 -print -quit)" \
    && -z "$(git -C "$repository" config --get core.hooksPath || true)" ]] || {
    printf 'isolated cargo-deny advisory DB contains alternates or hooks\n' >&2
    return 1
  }
  git -C "$repository" symbolic-ref -q HEAD >/dev/null 2>&1 && {
    printf 'isolated cargo-deny advisory DB HEAD must be detached\n' >&2
    return 1
  }
  [[ "$(git -C "$repository" rev-parse HEAD)" == "$expected_commit" \
    && "$(git -C "$repository" rev-parse 'HEAD^{tree}')" == "$expected_tree" \
    && -z "$(git -C "$repository" status --porcelain=v1)" ]] || {
    printf 'isolated cargo-deny advisory DB HEAD/tree/clean identity mismatch\n' >&2
    return 1
  }
  [[ -f "$repository/.git/FETCH_HEAD" && ! -L "$repository/.git/FETCH_HEAD" ]] || {
    printf 'isolated cargo-deny advisory DB lacks physical FETCH_HEAD\n' >&2
    return 1
  }
  mapfile -t fetch_lines <"$repository/.git/FETCH_HEAD"
  # The isolated clone is already proven to be detached at the pinned commit with
  # the pinned tree and a clean tree just above. FETCH_HEAD records whatever main
  # pointed at when the closure was seeded, which legitimately moves ahead of the
  # pin as RustSec publishes; requiring equality here re-introduced the same
  # mutable-position failure. What must hold is that the pinned commit is on the
  # lineage that was actually fetched.
  [[ "${#fetch_lines[@]}" -eq 1 ]] \
    && git -C "$repository" merge-base --is-ancestor \
      "$expected_commit" "${fetch_lines[0]%%$'\t'*}" || {
    printf 'isolated cargo-deny advisory DB FETCH_HEAD lineage mismatch\n' >&2
    return 1
  }
  [[ -z "$(git -C "$repository" remote)" \
    && -z "$(git -C "$repository" for-each-ref --format='%(refname)')" ]] || {
    printf 'isolated cargo-deny advisory DB contains unexpected refs or remotes\n' >&2
    return 1
  }
  expected_config=$'core.bare=false\ncore.filemode=true\ncore.logallrefupdates=true\ncore.repositoryformatversion=0'
  actual_config="$(git -C "$repository" config --local --list | LC_ALL=C sort)"
  [[ "$actual_config" == "$expected_config" ]] || {
    printf 'isolated cargo-deny advisory DB contains unexpected local config\n' >&2
    return 1
  }
  git -C "$repository" fsck --full --no-reflogs >/dev/null 2>&1 || {
    printf 'isolated cargo-deny advisory DB failed full fsck\n' >&2
    return 1
  }
}

jain_seed_cargo_deny_advisory_db() {
  local source_db="${1:?source advisory DB is required}"
  local cargo_home="${2:?isolated Cargo home is required}"
  local expected_commit="${3:?commit is required}"
  local expected_tree="${4:?tree is required}"
  local advisory_parent destination entry hook source_origin_main
  local -a advisory_entries=() hook_entries=() source_fetch_lines=()

  [[ -d "$source_db" && ! -L "$source_db" \
    && "$(realpath -e -- "$source_db")" == "$source_db" \
    && -d "$source_db/.git" && ! -L "$source_db/.git" ]] || {
    printf 'fixed cargo-deny advisory DB must be a physical checkout: %s\n' \
      "$source_db" >&2
    return 1
  }
  [[ -z "$(find "$source_db" -type l -print -quit)" \
    && ! -e "$source_db/.git/objects/info/alternates" \
    && -z "$(find "$source_db/.git/hooks" -type f ! -name '*.sample' -print -quit)" \
    && -z "$(git -C "$source_db" config --get core.hooksPath || true)" ]] || {
    printf 'fixed cargo-deny advisory DB contains symlinks, alternates, or active hooks\n' >&2
    return 1
  }
  source_origin_main="$(git -C "$source_db" rev-parse refs/remotes/origin/main 2>/dev/null)" \
    || return 1
  [[ -f "$source_db/.git/FETCH_HEAD" && ! -L "$source_db/.git/FETCH_HEAD" ]] || {
    printf 'fixed cargo-deny advisory DB lacks physical FETCH_HEAD\n' >&2
    return 1
  }
  mapfile -t source_fetch_lines <"$source_db/.git/FETCH_HEAD"
  # Verify the pinned OBJECT, not the checkout's mutable position. The advisory
  # database is a live upstream feed, so requiring HEAD/origin-main/FETCH_HEAD to
  # equal the pin failed the moment RustSec published anything — a false positive
  # on correct behaviour, not a tamper signal. What actually needs to hold is that
  # the pinned commit is present, carries the pinned tree, and sits on the real
  # upstream main lineage; the clone below then checks that exact commit out. This
  # mirrors how ops/ci/security.sh materialises its RustSec snapshot by archiving
  # the pinned commit rather than demanding HEAD be parked on it.
  [[ "$(git -C "$source_db" cat-file -t "$expected_commit" 2>/dev/null)" == "commit" \
    && "$(git -C "$source_db" rev-parse "${expected_commit}^{tree}")" == "$expected_tree" \
    && -n "$source_origin_main" \
    && "${#source_fetch_lines[@]}" -ge 1 \
    && -z "$(git -C "$source_db" status --porcelain=v1)" ]] \
    && git -C "$source_db" merge-base --is-ancestor \
      "$expected_commit" refs/remotes/origin/main || {
    printf 'fixed cargo-deny advisory DB pinned-object/lineage/clean identity mismatch\n' >&2
    return 1
  }
  git -C "$source_db" fsck --full --no-reflogs >/dev/null 2>&1 || {
    printf 'fixed cargo-deny advisory DB failed full fsck\n' >&2
    return 1
  }
  [[ -d "$cargo_home" && ! -L "$cargo_home" \
    && "$(realpath -e -- "$cargo_home")" == "$cargo_home" ]] || {
    printf 'cargo-deny advisory destination home must be physical: %s\n' \
      "$cargo_home" >&2
    return 1
  }

  advisory_parent="$cargo_home/advisory-dbs"
  if [[ -e "$advisory_parent" || -L "$advisory_parent" ]]; then
    [[ -d "$advisory_parent" && ! -L "$advisory_parent" ]] || {
      printf 'cargo-deny advisory parent must be a physical directory\n' >&2
      return 1
    }
    mapfile -t advisory_entries < <(find "$advisory_parent" -mindepth 1 -maxdepth 1 -print)
    for entry in "${advisory_entries[@]}"; do
      [[ "$entry" == "$advisory_parent/db.lock" && -f "$entry" && ! -L "$entry" ]] || {
        printf 'cargo-deny advisory parent contains an unexpected entry: %s\n' "$entry" >&2
        return 1
      }
    done
  else
    mkdir "$advisory_parent"
  fi
  destination="$advisory_parent/advisory-db-3157b0e258782691"
  [[ ! -e "$destination" && ! -L "$destination" ]] || {
    printf 'cargo-deny advisory destination already exists: %s\n' "$destination" >&2
    return 1
  }
  git -c core.hooksPath=/dev/null clone \
    --no-local --no-checkout --no-tags --single-branch --branch main \
    "$source_db" "$destination" >/dev/null
  git -C "$destination" fetch --no-tags "$source_db" refs/heads/main >/dev/null
  git -c core.hooksPath=/dev/null -C "$destination" \
    checkout --detach "$expected_commit" >/dev/null
  git -C "$destination" remote remove origin
  git -C "$destination" symbolic-ref -d refs/remotes/origin/HEAD >/dev/null 2>&1 || true
  git -C "$destination" update-ref -d refs/heads/main

  mapfile -t hook_entries < <(find "$destination/.git/hooks" -mindepth 1 -maxdepth 1 -print)
  for hook in "${hook_entries[@]}"; do
    [[ -f "$hook" && ! -L "$hook" && "$hook" == *.sample ]] || {
      printf 'isolated cargo-deny advisory clone created an unexpected hook: %s\n' \
        "$hook" >&2
      return 1
    }
    rm -- "$hook"
  done

  jain_verify_isolated_cargo_deny_db \
    "$destination" "$expected_commit" "$expected_tree" || return 1
  log "security: materialized isolated cargo-deny advisory DB commit=$expected_commit tree=$expected_tree"
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
