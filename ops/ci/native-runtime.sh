#!/usr/bin/env bash
# Shared runtime-library contract for release Cargo commands that enable the
# split family's native learners. This file is sourced by split-host-ci.sh.

jain_validate_native_build_tools() {
  local authority="${1:?native build-tool authority is required}"
  local bundle_root="${2:?native build-tool root is required}"
  local ownership_mode="${3:-content}"
  local expected_inventory expected_count actual_inventory actual_count
  local relative tool expected path version
  local -a tools=(cmake ninja ragel yasm)

  [[ "$ownership_mode" == root || "$ownership_mode" == content ]] || return 1
  [[ -f "$authority" && ! -L "$authority" ]] || {
    printf 'native build-tool authority is missing or linked: %s\n' \
      "$authority" >&2
    return 1
  }
  jq -e '
    select(type == "object")
    | select((keys | sort) ==
        ["bundle_root", "file_count", "inventory_sha256",
         "schema_version", "tools"])
    | select(.schema_version == "jain.native-build-tools/v1")
    | select(.bundle_root
        | test("^/var/lib/jain-host-ci/native-build-tools/[0-9a-f]{64}$"))
    | select(.inventory_sha256 | test("^[0-9a-f]{64}$"))
    | select(.bundle_root ==
        ("/var/lib/jain-host-ci/native-build-tools/" + .inventory_sha256))
    | select(.file_count | type == "number" and . > 0 and floor == .)
    | select((.tools | keys | sort) == ["cmake", "ninja", "ragel", "yasm"])
    | select(all(.tools[];
        type == "object"
        and (keys | sort) == ["mode", "path", "sha256", "size", "version"]
        and (.path | test("^bin/[a-z0-9-]+$"))
        and .mode == "555"
        and (.size | type == "number" and . > 0 and floor == .)
        and (.sha256 | test("^[0-9a-f]{64}$"))
        and (.version | type == "string" and length > 0)))' \
    "$authority" >/dev/null || {
    printf 'native build-tool authority is malformed\n' >&2
    return 1
  }

  expected_inventory="$(jq -er '.inventory_sha256' "$authority")" || return 1
  expected_count="$(jq -er '.file_count' "$authority")" || return 1
  [[ "$bundle_root" == /* && -d "$bundle_root" && ! -L "$bundle_root" \
    && "$(realpath -e -- "$bundle_root")" == "$bundle_root" \
    && "${bundle_root##*/}" == "$expected_inventory" ]] || {
    printf 'native build-tool bundle identity is invalid: %s\n' \
      "$bundle_root" >&2
    return 1
  }
  [[ -z "$(find "$bundle_root" -xdev ! -type d ! -type f -print -quit)" ]] || {
    printf 'native build-tool bundle contains a non-regular object\n' >&2
    return 1
  }
  [[ -z "$(find "$bundle_root" -xdev -type d ! -perm 0555 -print -quit)" \
    && -z "$(find "$bundle_root" -xdev -type f \
      ! \( -perm 0444 -o -perm 0555 \) -print -quit)" \
    && -z "$(find "$bundle_root" -xdev -type f ! -links 1 -print -quit)" ]] || {
    printf 'native build-tool bundle mode or link count is invalid\n' >&2
    return 1
  }
  if [[ "$ownership_mode" == root ]]; then
    [[ -z "$(find "$bundle_root" -xdev ! -uid 0 -print -quit)" \
      && -z "$(find "$bundle_root" -xdev ! -gid 0 -print -quit)" ]] || {
      printf 'native build-tool bundle is not root-owned\n' >&2
      return 1
    }
  fi

  actual_count="$(find "$bundle_root" -xdev -type f -printf '.\n' | wc -l)"
  [[ "$actual_count" == "$expected_count" ]] || {
    printf 'native build-tool file count mismatch: %s != %s\n' \
      "$actual_count" "$expected_count" >&2
    return 1
  }
  actual_inventory="$({
    while IFS= read -r relative; do
      [[ "$relative" =~ ^[A-Za-z0-9._+/-]+$ ]] || exit 1
      path="$bundle_root/$relative"
      printf '%s\t%s\t%s\t%s\n' "$relative" \
        "$(stat -c %a -- "$path")" "$(stat -c %s -- "$path")" \
        "$(sha256sum -- "$path" | cut -d' ' -f1)"
    done < <(LC_ALL=C find "$bundle_root" -xdev -type f -printf '%P\n' \
      | LC_ALL=C sort)
  } | sha256sum | cut -d' ' -f1)" || return 1
  [[ "$actual_inventory" == "$expected_inventory" ]] || {
    printf 'native build-tool inventory mismatch: %s != %s\n' \
      "$actual_inventory" "$expected_inventory" >&2
    return 1
  }

  for tool in "${tools[@]}"; do
    relative="$(jq -er --arg tool "$tool" '.tools[$tool].path' \
      "$authority")" || return 1
    path="$bundle_root/$relative"
    [[ -f "$path" && ! -L "$path" \
      && "$(stat -c %a -- "$path")" \
        == "$(jq -er --arg tool "$tool" '.tools[$tool].mode' "$authority")" \
      && "$(stat -c %s -- "$path")" \
        == "$(jq -er --arg tool "$tool" '.tools[$tool].size' "$authority")" \
      && "$(sha256sum -- "$path" | cut -d' ' -f1)" \
        == "$(jq -er --arg tool "$tool" '.tools[$tool].sha256' "$authority")" ]] \
      || {
      printf 'native build-tool digest/metadata mismatch: %s\n' "$tool" >&2
      return 1
    }
    expected="$(jq -er --arg tool "$tool" '.tools[$tool].version' \
      "$authority")" || return 1
    version="$("$path" --version 2>&1 | head -n 1)" || return 1
    [[ "$version" == "$expected" ]] || {
      printf 'native build-tool version mismatch: %s\n' "$tool" >&2
      return 1
    }
  done
}

jain_activate_native_build_tools() {
  local authority="${1:?native build-tool authority is required}"
  local bundle_root="${2:?native build-tool root is required}"
  jain_validate_native_build_tools "$authority" "$bundle_root" content \
    || return 1
  CMAKE="$bundle_root/bin/cmake"
  NINJA="$bundle_root/bin/ninja"
  CMAKE_MAKE_PROGRAM="$bundle_root/bin/ninja"
  PATH="$bundle_root/bin:$PATH"
  export CMAKE NINJA CMAKE_MAKE_PROGRAM PATH
}

jain_native_learners_for_repo() {
  local repo="${1:?repository name is required}"
  case "$repo" in
    jain-catboost) printf '%s\n' catboost ;;
    jain-xgboost) printf '%s\n' xgboost ;;
    jain-lightgbm) printf '%s\n' lightgbm ;;
    jain-core | jain-cli | jain-web | jain-deploy)
      printf '%s\n' catboost xgboost lightgbm
      ;;
  esac
}

jain_authoritative_control_plane_remote() {
  local inventory="${1:?managed repository inventory is required}"
  jq -er '
    [.repositories[]
      | select(.kind == "control-plane" and .name == "jain-split-ops")]
    | select(length == 1)
    | .[0].remote
    | select(type == "string" and length > 0)' <<<"$inventory"
}

jain_authoritative_required_check() {
  local inventory="${1:?managed repository inventory is required}"
  local repo="${2:?repository name is required}"
  jq -er --arg repo "$repo" '
    [.repositories[] | select(.name == $repo)]
    | select(length == 1)
    | .[0].required_check
    | select(type == "string" and length > 0)' <<<"$inventory"
}

jain_verify_reviewed_control_plane_commit() {
  local ops_root="${1:?control-plane root is required}"
  local expected_commit="${2:?control-plane commit is required}"
  local reviewed_remote="${3:?reviewed control-plane remote is required}"
  local authority_mode="${4:-remote}"
  local commit remote reviewed_commit

  [[ "$authority_mode" == remote || "$authority_mode" == local ]] || return 1

  [[ "$expected_commit" =~ ^[0-9a-f]{40}$ ]] || return 1
  commit="$(git -C "$ops_root" rev-parse --verify 'HEAD^{commit}' 2>/dev/null)" || {
    printf 'cannot resolve exact control-plane commit: %s\n' "$ops_root" >&2
    return 1
  }
  [[ "$commit" == "$expected_commit" ]] || {
    printf 'control-plane commit changed: %s != %s\n' \
      "$commit" "$expected_commit" >&2
    return 1
  }
  remote="$(git -C "$ops_root" remote get-url origin 2>/dev/null)" || {
    printf 'control-plane origin is unavailable: %s\n' "$ops_root" >&2
    return 1
  }
  [[ "$remote" == "$reviewed_remote" ]] || {
    printf 'control-plane origin does not match reviewed authority: %s != %s\n' \
      "$remote" "$reviewed_remote" >&2
    return 1
  }
  # The untrusted host-CI worker is deliberately network-isolated. It verifies
  # the exact local origin binding here; the root publisher independently reads
  # reviewed origin/main after the worker namespace has exited.
  [[ "$authority_mode" == local ]] && return 0
  reviewed_commit="$(GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 \
    git ls-remote --exit-code "$reviewed_remote" refs/heads/main 2>/dev/null \
    | cut -f1)" || {
    printf 'cannot read reviewed control-plane main from origin\n' >&2
    return 1
  }
  [[ "$commit" == "$reviewed_commit" ]] || {
    printf 'control-plane commit is not reviewed origin/main: %s != %s\n' \
      "$commit" "$reviewed_commit" >&2
    return 1
  }
}

jain_validate_native_check_mode() {
  local repo="${1:?repository name is required}"
  local check="${2:?check name is required}"
  local protected_check="${3:?protected check name is required}"
  local release_mode="${4:-0}"
  local -a learners=()
  mapfile -t learners < <(jain_native_learners_for_repo "$repo")
  if [[ "${#learners[@]}" -gt 0 && "$check" == "$protected_check" \
    && "$release_mode" != 1 ]]; then
    printf '%s cannot satisfy protected %s without JAIN_RELEASE_CI=1\n' \
      "$repo" "$protected_check" >&2
    return 1
  fi
}

jain_native_check_requires_evidence() {
  local repo="${1:?repository name is required}"
  local check="${2:?check name is required}"
  local protected_check="${3:?protected check name is required}"
  local -a learners=()
  mapfile -t learners < <(jain_native_learners_for_repo "$repo")
  [[ "${#learners[@]}" -gt 0 && "$check" == "$protected_check" ]]
}

jain_native_build_dirs() {
  local vendor_root="${1:?native vendor root is required}"
  printf '%s\n' \
    "$vendor_root/.build/catboost-nopy" \
    "$vendor_root/.build/xgboost-build" \
    "$vendor_root/.build/lightgbm-build"
}

jain_native_runtime_dirs() {
  local vendor_root="${1:?native vendor root is required}"
  printf '%s\n' \
    "$vendor_root/.build/catboost-nopy/catboost/libs/train_interface" \
    "$vendor_root/.build/xgboost-build" \
    "$vendor_root/.build/lightgbm-build"
}

jain_native_library_path() {
  local vendor_root="${1:?native vendor root is required}"
  local learner="${2:?native learner is required}"
  case "$learner" in
    catboost)
      printf '%s\n' \
        "$vendor_root/.build/catboost-nopy/catboost/libs/train_interface/libcatboost.so"
      ;;
    xgboost) printf '%s\n' "$vendor_root/.build/xgboost-build/libxgboost.so" ;;
    lightgbm) printf '%s\n' "$vendor_root/.build/lightgbm-build/lib_lightgbm.so" ;;
    *)
      printf 'unknown native learner: %s\n' "$learner" >&2
      return 1
      ;;
  esac
}

# Extract the native materializer and its authority from the exact control-plane
# commit. Mutable working-tree copies are deliberately not executable inputs.
# On success, the three JAIN_NATIVE_* variables below name the verified bundle.
jain_extract_native_materializer() {
  local ops_root="${1:?control-plane root is required}"
  local destination="${2:?materializer destination is required}"
  local reviewed_remote="${3:?reviewed control-plane remote is required}"
  local expected_commit="${4:-}"
  local authority_mode="${5:-remote}"
  local commit path expected actual
  local -a paths=(
    ops/ci/native-materializer.sh
    ops/ci/native-sources.lock.json
    ops/ci/native-sources.lock.json.sha256
  )

  case "$destination" in
    /*) ;;
    *)
      printf 'native materializer destination must be absolute: %s\n' "$destination" >&2
      return 1
      ;;
  esac
  commit="$(git -C "$ops_root" rev-parse --verify 'HEAD^{commit}' 2>/dev/null)" || {
    printf 'cannot resolve exact control-plane commit: %s\n' "$ops_root" >&2
    return 1
  }
  if [[ -n "$expected_commit" && "$commit" != "$expected_commit" ]]; then
    printf 'control-plane materializer commit changed: %s != %s\n' \
      "$commit" "$expected_commit" >&2
    return 1
  fi
  jain_verify_reviewed_control_plane_commit \
    "$ops_root" "$commit" "$reviewed_remote" "$authority_mode" || return 1
  for path in "${paths[@]}"; do
    git -C "$ops_root" cat-file -e "$commit:$path" 2>/dev/null || {
      printf 'reviewed control-plane commit lacks native input: %s\n' "$path" >&2
      return 1
    }
    if [[ -n "$(git -C "$ops_root" status --porcelain=v1 --untracked-files=all -- "$path")" ]]; then
      printf 'mutable control-plane native input is not allowed: %s\n' "$path" >&2
      return 1
    fi
  done

  mkdir -p "$destination"
  for path in "${paths[@]}"; do
    git -C "$ops_root" show "$commit:$path" >"$destination/${path##*/}" || return 1
  done
  (
    cd "$destination"
    sha256sum -c native-sources.lock.json.sha256 >/dev/null
  ) || {
    printf 'native source authority sidecar verification failed\n' >&2
    return 1
  }
  expected="$(jq -er \
    '.materializer.path == "ops/ci/native-materializer.sh"
     and (.materializer.sha256 | test("^[0-9a-f]{64}$"))
     | select(.)' "$destination/native-sources.lock.json" >/dev/null && \
    jq -er '.materializer.sha256' "$destination/native-sources.lock.json")" || {
    printf 'native source authority has an invalid materializer binding\n' >&2
    return 1
  }
  actual="$(sha256sum -- "$destination/native-materializer.sh" | cut -d' ' -f1)"
  [[ "$actual" == "$expected" ]] || {
    printf 'reviewed native materializer digest mismatch: expected %s, got %s\n' \
      "$expected" "$actual" >&2
    return 1
  }
  chmod 0555 "$destination/native-materializer.sh"
  chmod 0444 "$destination/native-sources.lock.json" \
    "$destination/native-sources.lock.json.sha256"
  JAIN_NATIVE_CONTROL_COMMIT="$commit"
  JAIN_NATIVE_MATERIALIZER="$destination/native-materializer.sh"
  JAIN_NATIVE_AUTHORITY="$destination/native-sources.lock.json"
  export JAIN_NATIVE_CONTROL_COMMIT JAIN_NATIVE_MATERIALIZER JAIN_NATIVE_AUTHORITY
}

jain_native_source_root() {
  local input="${1:?native source root is required}"
  if [[ -d "$input/vendor/catboost" ]]; then
    printf '%s\n' "$input/vendor"
  elif [[ -d "$input/catboost" ]]; then
    printf '%s\n' "$input"
  else
    printf 'native source root must contain catboost/, xgboost/, and lightgbm/: %s\n' \
      "$input" >&2
    return 1
  fi
}

# Run Git directly against a validated object database. This deliberately
# avoids repository discovery and mutable worktree reads, so a release worker
# can inspect root-owned native custody without granting wildcard trust.
jain_native_git_object() {
  local git_dir="${1:?native Git object database is required}"
  local config_variable
  shift
  (
    export GIT_CONFIG_NOSYSTEM=1
    export GIT_CONFIG_GLOBAL=/dev/null
    unset GIT_CONFIG_PARAMETERS GIT_CONFIG_COUNT GIT_CONFIG_SYSTEM
    for config_variable in "${!GIT_CONFIG_KEY_@}" "${!GIT_CONFIG_VALUE_@}"; do
      [[ -n "$config_variable" ]] && unset "$config_variable"
    done
    git --git-dir="$git_dir" \
      -c core.fsmonitor=false -c core.hooksPath=/dev/null "$@"
  )
}

jain_native_object_tree_is_symlink_free() {
  local git_dir="${1:?native Git object database is required}"
  local revision="${2:?native revision is required}"
  jain_native_git_object "$git_dir" ls-tree -r --full-tree "$revision" \
    | awk '$1 == "120000" {found=1} END {exit found}'
}

jain_native_physical_object_database() {
  local repository="${1:?native source repository is required}"
  local repository_real git_dir git_dir_real actual_git_dir common_dir is_bare
  case "$repository" in
    /*) ;;
    *)
      printf 'native source repository must be absolute: %s\n' "$repository" >&2
      return 1
      ;;
  esac
  # The exact path is later embedded in a command-scoped upload-pack command.
  # Reject shell metacharacters and ambiguous spellings before that boundary.
  [[ "$repository" =~ ^/[A-Za-z0-9._/-]+$ \
    && "$repository" != *'//'*
    && -d "$repository" && ! -L "$repository" ]] || {
    printf 'native source repository is not a safe physical path: %s\n' \
      "$repository" >&2
    return 1
  }
  repository_real="$(realpath -e -- "$repository" 2>/dev/null)" || return 1
  [[ "$repository_real" == "$repository" ]] || {
    printf 'native source repository is aliased: %s\n' "$repository" >&2
    return 1
  }
  git_dir="$repository/.git"
  [[ -d "$git_dir" && ! -L "$git_dir" \
    && ! -e "$git_dir/commondir" && ! -L "$git_dir/commondir" \
    && ! -e "$git_dir/worktrees" && ! -L "$git_dir/worktrees" \
    && ! -e "$git_dir/objects/info/alternates" \
    && ! -L "$git_dir/objects/info/alternates" ]] || {
    printf 'native source Git database is not an independent primary: %s\n' \
      "$repository" >&2
    return 1
  }
  git_dir_real="$(realpath -e -- "$git_dir" 2>/dev/null)" || return 1
  [[ "$git_dir_real" == "$git_dir" ]] || {
    printf 'native source Git database is aliased: %s\n' "$git_dir" >&2
    return 1
  }
  actual_git_dir="$(jain_native_git_object "$git_dir" \
    rev-parse --path-format=absolute --absolute-git-dir 2>/dev/null)" || return 1
  common_dir="$(jain_native_git_object "$git_dir" \
    rev-parse --path-format=absolute --git-common-dir 2>/dev/null)" || return 1
  is_bare="$(jain_native_git_object "$git_dir" \
    rev-parse --is-bare-repository 2>/dev/null)" || return 1
  [[ "$actual_git_dir" == "$git_dir" && "$common_dir" == "$git_dir" \
    && "$is_bare" == false ]] || {
    printf 'native source Git database identity is invalid: %s\n' "$git_dir" >&2
    return 1
  }
  printf '%s\n' "$git_dir"
}

# Resolve a populated submodule's physical object database without asking Git
# to discover the foreign-owned worktree. The gitfile may only resolve inside
# the already validated learner repository's private modules directory.
jain_native_submodule_object_database() {
  local repository="${1:?native submodule repository is required}"
  local root_git_dir="${2:?native learner Git database is required}"
  local repository_real root_git_dir_real modules_root git_file git_dir
  local actual_git_dir common_dir is_bare
  local -a git_file_lines=()

  [[ "$repository" =~ ^/[A-Za-z0-9._/-]+$ \
    && "$repository" != *'//'* \
    && -d "$repository" && ! -L "$repository" \
    && "$root_git_dir" =~ ^/[A-Za-z0-9._/-]+$ \
    && "$root_git_dir" != *'//'* \
    && -d "$root_git_dir" && ! -L "$root_git_dir" ]] || {
    printf 'native submodule path is not physically safe: %s\n' \
      "$repository" >&2
    return 1
  }
  repository_real="$(realpath -e -- "$repository" 2>/dev/null)" || return 1
  root_git_dir_real="$(realpath -e -- "$root_git_dir" 2>/dev/null)" || return 1
  [[ "$repository_real" == "$repository" \
    && "$root_git_dir_real" == "$root_git_dir" ]] || {
    printf 'native submodule path is aliased: %s\n' "$repository" >&2
    return 1
  }

  git_file="$repository/.git"
  [[ -f "$git_file" && ! -L "$git_file" ]] || {
    printf 'native submodule lacks a physical gitfile: %s\n' "$repository" >&2
    return 1
  }
  mapfile -t git_file_lines <"$git_file"
  [[ "${#git_file_lines[@]}" -eq 1 \
    && "${git_file_lines[0]}" =~ ^gitdir:\ ([A-Za-z0-9._/-]+)$ ]] || {
    printf 'native submodule gitfile is malformed: %s\n' "$git_file" >&2
    return 1
  }
  git_dir="$(realpath -e -- "$repository/${BASH_REMATCH[1]}" 2>/dev/null)" \
    || return 1
  modules_root="$(realpath -e -- "$root_git_dir/modules" 2>/dev/null)" || return 1
  [[ "$modules_root" == "$root_git_dir/modules" \
    && "$git_dir" == "$modules_root/"* \
    && -d "$git_dir" && ! -L "$git_dir" \
    && ! -e "$git_dir/commondir" && ! -L "$git_dir/commondir" \
    && ! -e "$git_dir/worktrees" && ! -L "$git_dir/worktrees" \
    && ! -e "$git_dir/objects/info/alternates" \
    && ! -L "$git_dir/objects/info/alternates" ]] || {
    printf 'native submodule object database escaped learner custody: %s\n' \
      "$repository" >&2
    return 1
  }
  actual_git_dir="$(jain_native_git_object "$git_dir" \
    rev-parse --path-format=absolute --absolute-git-dir 2>/dev/null)" || return 1
  common_dir="$(jain_native_git_object "$git_dir" \
    rev-parse --path-format=absolute --git-common-dir 2>/dev/null)" || return 1
  is_bare="$(jain_native_git_object "$git_dir" \
    rev-parse --is-bare-repository 2>/dev/null)" || return 1
  [[ "$actual_git_dir" == "$git_dir" && "$common_dir" == "$git_dir" \
    && "$is_bare" == false ]] || {
    printf 'native submodule Git database identity is invalid: %s\n' \
      "$git_dir" >&2
    return 1
  }
  printf '%s\n' "$git_dir"
}

jain_clone_native_object_database() {
  local git_dir="${1:?native Git object database is required}"
  local destination="${2:?native clone destination is required}"
  local config_variable
  [[ "$git_dir" =~ ^/[A-Za-z0-9._/-]+$ && "$git_dir" != *'//'* \
    && -d "$git_dir" && ! -L "$git_dir" && "$destination" == /* ]] || return 1
  (
    export GIT_CONFIG_NOSYSTEM=1
    export GIT_CONFIG_GLOBAL=/dev/null
    unset GIT_CONFIG_PARAMETERS GIT_CONFIG_COUNT GIT_CONFIG_SYSTEM
    for config_variable in "${!GIT_CONFIG_KEY_@}" "${!GIT_CONFIG_VALUE_@}"; do
      [[ -n "$config_variable" ]] && unset "$config_variable"
    done
    git clone --quiet --no-local --no-checkout \
      --upload-pack="git -c safe.directory=$git_dir upload-pack" \
      "$git_dir" "$destination"
  )
}

# Create clean detached physical clones at the authority revisions without
# registering linked checkouts or retaining object alternates to the canonical
# source repositories. Dirty checkout bytes are never read. Git object, tree,
# and SHA-256 tree-manifest identities are verified before checkout.
jain_stage_native_source_worktrees() {
  local authority="${1:?native source authority is required}"
  local source_input="${2:?native source root is required}"
  local staged_root="${3:?staged native source root is required}"
  local source_root learner revision tree manifest actual object_database
  local learner_object_database
  local sub_count index sub_path sub_revision sub_tree sub_manifest
  local -a learners=()

  source_root="$(jain_native_source_root "$source_input")" || return 1
  case "$staged_root" in
    /*) ;;
    *)
      printf 'staged native source root must be absolute: %s\n' "$staged_root" >&2
      return 1
      ;;
  esac
  [[ ! -e "$staged_root" ]] || {
    printf 'staged native source root already exists: %s\n' "$staged_root" >&2
    return 1
  }
  mkdir -p "$staged_root"
  mapfile -t learners < <(jq -er '.learners[].name' "$authority")
  [[ "${#learners[@]}" -eq 3 ]] || return 1

  for learner in "${learners[@]}"; do
    revision="$(jq -er --arg learner "$learner" \
      '.learners[] | select(.name == $learner) | .revision' "$authority")" || return 1
    tree="$(jq -er --arg learner "$learner" \
      '.learners[] | select(.name == $learner) | .git_tree' "$authority")" || return 1
    manifest="$(jq -er --arg learner "$learner" \
      '.learners[] | select(.name == $learner) | .tree_manifest_sha256' \
      "$authority")" || return 1
    [[ -d "$source_root/$learner" ]] || {
      printf 'native source repository is missing: %s\n' "$source_root/$learner" >&2
      jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
      return 1
    }
    object_database="$(jain_native_physical_object_database \
      "$source_root/$learner" 2>/dev/null || true)"
    [[ -n "$object_database" ]] || {
      printf '%s native object source is not a physical primary repository\n' \
        "$learner" >&2
      jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
      return 1
    }
    learner_object_database="$object_database"
    actual="$(jain_native_git_object "$object_database" \
      rev-parse --verify "$revision^{commit}" 2>/dev/null || true)"
    [[ "$actual" == "$revision" ]] || {
      printf '%s native object source revision is unavailable: %s\n' \
        "$learner" "$revision" >&2
      jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
      return 1
    }
    actual="$(jain_native_git_object "$object_database" \
      rev-parse --verify "$revision^{tree}" 2>/dev/null || true)"
    [[ "$actual" == "$tree" ]] || {
      printf '%s native object source tree mismatch\n' "$learner" >&2
      jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
      return 1
    }
    actual="$(jain_native_git_object "$object_database" \
      ls-tree -r --full-tree "$revision" \
      | sha256sum | cut -d' ' -f1)"
    [[ "$actual" == "$manifest" ]] || {
      printf '%s native object source manifest mismatch\n' "$learner" >&2
      jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
      return 1
    }
    if ! jain_native_object_tree_is_symlink_free \
      "$object_database" "$revision"; then
      printf '%s native source tree contains a prohibited symlink\n' \
        "$learner" >&2
      jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
      return 1
    fi
    jain_clone_native_object_database "$object_database" \
      "$staged_root/$learner" || {
      jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
      return 1
    }
    git -C "$staged_root/$learner" checkout --quiet --detach "$revision" || {
      jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
      return 1
    }
    git -C "$staged_root/$learner" remote remove origin || return 1
    [[ -d "$staged_root/$learner/.git" \
      && ! -e "$staged_root/$learner/.git/commondir" \
      && ! -e "$staged_root/$learner/.git/worktrees" \
      && ! -e "$staged_root/$learner/.git/objects/info/alternates" \
      && "$(git -C "$staged_root/$learner" rev-parse --path-format=absolute --absolute-git-dir)" \
        == "$staged_root/$learner/.git" \
      && "$(git -C "$staged_root/$learner" rev-parse --path-format=absolute --git-common-dir)" \
        == "$staged_root/$learner/.git" \
      && -z "$(find "$staged_root/$learner" -xdev -type l -print -quit)" \
      && -z "$(find "$staged_root/$learner" -xdev ! -type d ! -type f -print -quit)" ]] || {
      jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
      return 1
    }
    sub_count="$(jq -er --arg learner "$learner" \
      '.learners[] | select(.name == $learner) | (.submodules // []) | length' \
      "$authority")" || return 1
    for ((index = 0; index < sub_count; index++)); do
      sub_path="$(jq -er --arg learner "$learner" --argjson index "$index" \
        '.learners[] | select(.name == $learner) | .submodules[$index].path' \
        "$authority")" || return 1
      sub_revision="$(jq -er --arg learner "$learner" --argjson index "$index" \
        '.learners[] | select(.name == $learner) | .submodules[$index].revision' \
        "$authority")" || return 1
      sub_tree="$(jq -er --arg learner "$learner" --argjson index "$index" \
        '.learners[] | select(.name == $learner) | .submodules[$index].git_tree' \
        "$authority")" || return 1
      sub_manifest="$(jq -er --arg learner "$learner" --argjson index "$index" \
        '.learners[] | select(.name == $learner) | .submodules[$index].tree_manifest_sha256' \
        "$authority")" || return 1
      [[ -d "$source_root/$learner/$sub_path" ]] || {
        printf '%s pinned submodule object source is missing: %s\n' \
          "$learner" "$sub_path" >&2
        jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
        return 1
      }
      object_database="$(jain_native_submodule_object_database \
        "$source_root/$learner/$sub_path" \
        "$learner_object_database" 2>/dev/null || true)"
      [[ -n "$object_database" ]] || {
        printf '%s pinned submodule is not a physical primary: %s\n' \
          "$learner" "$sub_path" >&2
        jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
        return 1
      }
      actual="$(jain_native_git_object "$object_database" \
        rev-parse --verify "$sub_revision^{commit}" 2>/dev/null || true)"
      [[ "$actual" == "$sub_revision" ]] || {
        printf '%s pinned submodule revision is unavailable: %s\n' \
          "$learner" "$sub_path" >&2
        jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
        return 1
      }
      actual="$(jain_native_git_object "$object_database" \
        rev-parse --verify "$sub_revision^{tree}" 2>/dev/null || true)"
      [[ "$actual" == "$sub_tree" ]] || {
        printf '%s pinned submodule tree mismatch: %s\n' "$learner" "$sub_path" >&2
        jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
        return 1
      }
      actual="$(jain_native_git_object "$object_database" \
        ls-tree -r --full-tree "$sub_revision" | sha256sum | cut -d' ' -f1)"
      [[ "$actual" == "$sub_manifest" ]] || {
        printf '%s pinned submodule manifest mismatch: %s\n' "$learner" "$sub_path" >&2
        jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
        return 1
      }
      if ! jain_native_object_tree_is_symlink_free \
        "$object_database" "$sub_revision"; then
        printf '%s pinned submodule tree contains a prohibited symlink: %s\n' \
          "$learner" "$sub_path" >&2
        jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
        return 1
      fi
      mkdir -p "$(dirname "$staged_root/$learner/$sub_path")"
      jain_clone_native_object_database "$object_database" \
        "$staged_root/$learner/$sub_path" || {
        jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
        return 1
      }
      git -C "$staged_root/$learner/$sub_path" checkout --quiet --detach \
        "$sub_revision" || {
        jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
        return 1
      }
      git -C "$staged_root/$learner/$sub_path" remote remove origin || return 1
      [[ -d "$staged_root/$learner/$sub_path/.git" \
        && ! -e "$staged_root/$learner/$sub_path/.git/commondir" \
        && ! -e "$staged_root/$learner/$sub_path/.git/worktrees" \
        && ! -e "$staged_root/$learner/$sub_path/.git/objects/info/alternates" \
        && "$(git -C "$staged_root/$learner/$sub_path" rev-parse --path-format=absolute --absolute-git-dir)" \
          == "$staged_root/$learner/$sub_path/.git" \
        && "$(git -C "$staged_root/$learner/$sub_path" rev-parse --path-format=absolute --git-common-dir)" \
          == "$staged_root/$learner/$sub_path/.git" \
        && -z "$(find "$staged_root/$learner/$sub_path" -xdev -type l -print -quit)" \
        && -z "$(find "$staged_root/$learner/$sub_path" -xdev ! -type d ! -type f -print -quit)" ]] || {
        jain_cleanup_native_source_worktrees "$authority" "$source_root" "$staged_root"
        return 1
      }
    done
  done
}

jain_cleanup_native_source_worktrees() {
  local authority="${1:?native source authority is required}"
  local source_input="${2:?native source root is required}"
  local staged_root="${3:?staged native source root is required}"
  : "$authority" "$source_input"
  [[ "$staged_root" == /* && "$staged_root" != / \
    && -z "$(find "$staged_root" -xdev -type l -print -quit 2>/dev/null)" \
    && -z "$(find "$staged_root" -xdev ! -type d ! -type f -print -quit 2>/dev/null)" ]] \
    || return 1
  rm -rf -- "$staged_root"
}

jain_write_sha256_sidecar() {
  local file="${1:?file is required}"
  (
    cd "$(dirname "$file")" || exit 1
    sha256sum "$(basename "$file")" >"$(basename "$file").sha256"
  )
}

jain_verify_sha256_sidecar() {
  local file="${1:?file is required}" expected actual basename
  [[ -f "$file" && ! -L "$file" \
    && "$(stat -c '%h' -- "$file" 2>/dev/null)" == 1 \
    && -f "$file.sha256" && ! -L "$file.sha256" \
    && "$(stat -c '%h' -- "$file.sha256" 2>/dev/null)" == 1 ]] || return 1
  basename="$(basename "$file")"
  expected="$(sed -n -E \
    "s/^([0-9a-f]{64})  ${basename//./\\.}\$/\\1/p" "$file.sha256")"
  [[ "$expected" =~ ^[0-9a-f]{64}$ \
    && "$(wc -l <"$file.sha256")" == 1 ]] || return 1
  actual="$(sha256sum -- "$file" | cut -d' ' -f1)"
  [[ "$actual" == "$expected" ]]
}

jain_control_plane_file_sha256() {
  local control_root="${1:?control-plane root is required}"
  local control_commit="${2:?control-plane commit is required}"
  local path="${3:?control-plane path is required}"
  git -C "$control_root" cat-file -e "$control_commit:$path" 2>/dev/null || return 1
  git -C "$control_root" show "$control_commit:$path" | sha256sum | cut -d' ' -f1
}

jain_copy_control_plane_file() {
  local control_root="${1:?control-plane root is required}"
  local control_commit="${2:?control-plane commit is required}"
  local path="${3:?control-plane path is required}"
  local destination="${4:?destination is required}"
  local expected actual
  expected="$(jain_control_plane_file_sha256 \
    "$control_root" "$control_commit" "$path")" || return 1
  git -C "$control_root" show "$control_commit:$path" >"$destination" || return 1
  actual="$(sha256sum -- "$destination" | cut -d' ' -f1)"
  [[ "$actual" == "$expected" ]] || {
    printf 'control-plane file changed while extracting: %s\n' "$path" >&2
    return 1
  }
}

jain_verify_evidence_control_plane_files() {
  local evidence_dir="${1:?native evidence directory is required}"
  local control_root="${2:?control-plane root is required}"
  local control_commit="${3:?control-plane commit is required}"
  local index expected actual
  local -a evidence_files=(
    native-sources.lock.json
    native-materializer.sh
    native-runtime.sh
    split-host-ci.sh
    host-ci-integrity.sh
  )
  local -a control_paths=(
    ops/ci/native-sources.lock.json
    ops/ci/native-materializer.sh
    ops/ci/native-runtime.sh
    ops/ci/split-host-ci.sh
    ops/ci/host-ci-integrity.sh
  )

  [[ "$control_commit" =~ ^[0-9a-f]{40}$ ]] || return 1
  for ((index = 0; index < ${#evidence_files[@]}; index++)); do
    expected="$(jain_control_plane_file_sha256 \
      "$control_root" "$control_commit" "${control_paths[$index]}")" || return 1
    actual="$(sha256sum -- "$evidence_dir/${evidence_files[$index]}" \
      | cut -d' ' -f1)"
    [[ "$actual" == "$expected" ]] || {
      printf 'native evidence is not from control-plane commit %s: %s\n' \
        "$control_commit" "${evidence_files[$index]}" >&2
      return 1
    }
  done
}

JAIN_NATIVE_EVIDENCE_MAX_FILES=16
JAIN_NATIVE_EVIDENCE_MAX_BYTES=16777216
JAIN_NATIVE_EVIDENCE_MAX_FILE_BYTES=8388608
JAIN_NATIVE_EVIDENCE_MIN_FREE_BYTES=1073741824
JAIN_NATIVE_EVIDENCE_RETAIN_PER_CHECK=8
readonly JAIN_NATIVE_EVIDENCE_MAX_FILES JAIN_NATIVE_EVIDENCE_MAX_BYTES
readonly JAIN_NATIVE_EVIDENCE_MAX_FILE_BYTES JAIN_NATIVE_EVIDENCE_MIN_FREE_BYTES
readonly JAIN_NATIVE_EVIDENCE_RETAIN_PER_CHECK

jain_native_evidence_files() {
  local file
  for file in materialization.log native-vendor-manifest.json \
    native-sources.lock.json native-materializer.sh native-runtime.sh \
    split-host-ci.sh host-ci-integrity.sh receipt.json; do
    printf '%s\n%s.sha256\n' "$file" "$file"
  done
}

jain_verify_native_evidence_layout() {
  local evidence_dir="${1:?native evidence directory is required}"
  local file size total=0
  local -a expected=() actual=()
  [[ -d "$evidence_dir" && ! -L "$evidence_dir" ]] || return 1
  mapfile -t expected < <(jain_native_evidence_files | LC_ALL=C sort)
  mapfile -t actual < <(
    find "$evidence_dir" -mindepth 1 -maxdepth 1 -printf '%f\n' | LC_ALL=C sort
  )
  [[ "${#expected[@]}" == "$JAIN_NATIVE_EVIDENCE_MAX_FILES" \
    && "${actual[*]}" == "${expected[*]}" ]] || return 1
  for file in "${expected[@]}"; do
    [[ -f "$evidence_dir/$file" && ! -L "$evidence_dir/$file" \
      && "$(stat -c '%h' -- "$evidence_dir/$file" 2>/dev/null)" == 1 ]] \
      || return 1
    size="$(stat -c '%s' -- "$evidence_dir/$file")" || return 1
    (( size <= JAIN_NATIVE_EVIDENCE_MAX_FILE_BYTES )) || return 1
    total=$((total + size))
    (( total <= JAIN_NATIVE_EVIDENCE_MAX_BYTES )) || return 1
  done
  printf '%s\n' "$total"
}

jain_verify_native_evidence() {
  local evidence_dir="${1:?native evidence directory is required}"
  local expected_sha="${2:-}" expected_check="${3:-}"
  local control_root="${4:?control-plane root is required}"
  local expected_control_commit="${5:?control-plane commit is required}"
  local file
  jain_verify_native_evidence_layout "$evidence_dir" >/dev/null || return 1
  for file in materialization.log native-vendor-manifest.json \
    native-sources.lock.json native-materializer.sh native-runtime.sh \
    split-host-ci.sh host-ci-integrity.sh receipt.json; do
    jain_verify_sha256_sidecar "$evidence_dir/$file" || return 1
  done
  jq -e --arg expected_sha "$expected_sha" --arg expected_check "$expected_check" \
    --arg expected_control_commit "$expected_control_commit" \
    'select(.schema_version == "jain.host-native-materialization/v1")
     | select(($expected_sha == "" or .head_sha == $expected_sha)
       and ($expected_check == "" or .required_check == $expected_check))
     | select(.head_sha | test("^[0-9a-f]{40}$"))
     | select(.control_plane_commit == $expected_control_commit)
     | select(.status == "pass")' "$evidence_dir/receipt.json" >/dev/null || return 1
  jain_verify_evidence_control_plane_files \
    "$evidence_dir" "$control_root" "$expected_control_commit" || return 1
  jq -e \
    --arg log "$(sha256sum -- "$evidence_dir/materialization.log" | cut -d' ' -f1)" \
    --arg manifest "$(sha256sum -- "$evidence_dir/native-vendor-manifest.json" | cut -d' ' -f1)" \
    --arg authority "$(sha256sum -- "$evidence_dir/native-sources.lock.json" | cut -d' ' -f1)" \
    --arg materializer "$(sha256sum -- "$evidence_dir/native-materializer.sh" | cut -d' ' -f1)" \
    --arg runtime "$(sha256sum -- "$evidence_dir/native-runtime.sh" | cut -d' ' -f1)" \
    --arg runner "$(sha256sum -- "$evidence_dir/split-host-ci.sh" | cut -d' ' -f1)" \
    --arg integrity "$(sha256sum -- "$evidence_dir/host-ci-integrity.sh" | cut -d' ' -f1)" \
    '.log_sha256 == $log
     and .native_vendor_manifest_sha256 == $manifest
     and .authority.sha256 == $authority
     and .materializer.sha256 == $materializer
     and .orchestration.native_runtime_sha256 == $runtime
     and .orchestration.split_host_ci_sha256 == $runner
     and .orchestration.host_ci_integrity_sha256 == $integrity' \
    "$evidence_dir/receipt.json" >/dev/null
}

jain_verify_native_evidence_binding() {
  local evidence_dir="${1:?native evidence directory is required}"
  local expected_receipt_sha="${2:?native receipt digest is required}"
  local expected_head="${3:-}" expected_check="${4:-}"
  local control_root="${5:?control-plane root is required}"
  local expected_control_commit="${6:?control-plane commit is required}"
  [[ "$expected_receipt_sha" =~ ^[0-9a-f]{64}$ ]] || return 1
  jain_verify_native_evidence "$evidence_dir" "$expected_head" "$expected_check" \
    "$control_root" "$expected_control_commit" || return 1
  [[ "$(sha256sum -- "$evidence_dir/receipt.json" | cut -d' ' -f1)" == \
    "$expected_receipt_sha" ]]
}

jain_verify_native_check_evidence() {
  local conclusion="${1:?check conclusion is required}"
  local evidence_required="${2:-0}"
  local evidence_dir="${3:-}" receipt_sha="${4:-}"
  local expected_head="${5:-}" expected_check="${6:-}"
  local control_root="${7:-}" expected_control_commit="${8:-}"
  if [[ -n "$evidence_dir" || -n "$receipt_sha" ]]; then
    [[ -n "$evidence_dir" && -n "$receipt_sha" ]] || return 1
    jain_verify_native_evidence_binding "$evidence_dir" "$receipt_sha" \
      "$expected_head" "$expected_check" \
      "$control_root" "$expected_control_commit"
    return
  fi
  if [[ "$evidence_required" == 1 && "$conclusion" == success ]]; then
    printf 'protected native success requires exact materialization evidence\n' >&2
    return 1
  fi
}

jain_resolve_durable_evidence_root() {
  local evidence_root="${1:?native evidence root is required}"
  local ephemeral_root="${2:?ephemeral CI root is required}"
  local evidence_resolved ephemeral_resolved verified
  command -v realpath >/dev/null 2>&1 || {
    printf 'native evidence requires realpath\n' >&2
    return 1
  }
  [[ "$evidence_root" = /* && "$ephemeral_root" = /* ]] || {
    printf 'native evidence and ephemeral roots must be absolute\n' >&2
    return 1
  }
  ephemeral_resolved="$(realpath -e -- "$ephemeral_root" 2>/dev/null)" || {
    printf 'ephemeral CI root cannot be resolved: %s\n' "$ephemeral_root" >&2
    return 1
  }
  evidence_resolved="$(realpath -m -- "$evidence_root")" || return 1
  case "$evidence_resolved" in
    /tmp | /tmp/*)
      printf 'native evidence root cannot use /tmp: %s\n' "$evidence_resolved" >&2
      return 1
      ;;
  esac
  case "$evidence_resolved" in
    "$ephemeral_resolved" | "$ephemeral_resolved"/*)
      printf 'native evidence root resolves inside ephemeral CI: %s\n' \
        "$evidence_resolved" >&2
      return 1
      ;;
  esac
  mkdir -p "$evidence_resolved" || return 1
  verified="$(realpath -e -- "$evidence_resolved" 2>/dev/null)" || return 1
  [[ "$verified" == "$evidence_resolved" ]] || {
    printf 'native evidence root changed while resolving: %s\n' "$evidence_root" >&2
    return 1
  }
  printf '%s\n' "$evidence_resolved"
}

jain_resolve_worker_evidence_staging_root() {
  local evidence_root="${1:?native evidence staging root is required}"
  local configured="${JAIN_NATIVE_EVIDENCE_STAGING_ROOT:-}"
  local writable="${JAIN_HOST_CI_WRITABLE_ROOT:-}"
  local expected resolved
  [[ -n "$configured" && -n "$writable" \
    && "$evidence_root" == "$configured" ]] || return 1
  expected="$(realpath -e -- "$writable")/native-evidence-staging" || return 1
  resolved="$(realpath -e -- "$evidence_root")" || return 1
  [[ "$resolved" == "$expected" && ! -L "$resolved" \
    && "$(stat -c '%u:%a' -- "$resolved")" == "$(id -u):700" \
    && "$(stat -f -c '%T' -- "$resolved")" == tmpfs ]] || {
    printf 'native evidence staging is outside the root quota boundary: %s\n' \
      "$evidence_root" >&2
    return 1
  }
  printf '%s\n' "$resolved"
}

# Persist all native inputs, output manifest, and log outside the disposable
# host-CI tree. The receipt digest is later included in the exact-SHA status.
jain_persist_native_evidence() {
  local vendor_root="${1:?native vendor root is required}"
  local log="${2:?native materialization log is required}"
  local evidence_root="${3:?native evidence root is required}"
  local ephemeral_root="${4:?ephemeral CI root is required}"
  local owner="${5:?owner is required}" repo="${6:?repository is required}"
  local head_sha="${7:?head SHA is required}" check="${8:?required check is required}"
  local control_commit="${9:?control-plane commit is required}"
  local control_root="${10:?control-plane root is required}"
  local check_slug attempt parent staging destination recorded_at file

  [[ "$head_sha" =~ ^[0-9a-f]{40}$ && "$control_commit" =~ ^[0-9a-f]{40}$ ]] || return 1
  [[ "$owner" =~ ^[A-Za-z0-9_.-]+$ && "$repo" =~ ^[A-Za-z0-9_.-]+$ ]] || return 1
  if [[ -n "${JAIN_NATIVE_EVIDENCE_STAGING_ROOT:-}" ]]; then
    evidence_root="$(jain_resolve_worker_evidence_staging_root \
      "$evidence_root")" || return 1
  else
    evidence_root="$(jain_resolve_durable_evidence_root \
      "$evidence_root" "$ephemeral_root")" || return 1
  fi
  [[ -s "$log" && -s "$vendor_root/receipts/manifest.json" ]] || return 1
  check_slug="${check//[^A-Za-z0-9_.-]/_}"
  attempt="${JAIN_CI_ATTEMPT_ID:-$(date -u +%Y%m%dT%H%M%SZ)-$$}"
  [[ "$attempt" =~ ^[A-Za-z0-9_.-]+$ ]] || return 1
  parent="$evidence_root/$owner/$repo/$head_sha/$check_slug"
  destination="$parent/$attempt"
  mkdir -p "$parent"
  [[ ! -e "$destination" ]] || return 1
  staging="$(mktemp -d "$parent/.staging.XXXXXX")"
  cp -- "$log" "$staging/materialization.log"
  cp -- "$vendor_root/receipts/manifest.json" "$staging/native-vendor-manifest.json"
  jain_copy_control_plane_file "$control_root" "$control_commit" \
    ops/ci/native-sources.lock.json "$staging/native-sources.lock.json" || return 1
  jain_copy_control_plane_file "$control_root" "$control_commit" \
    ops/ci/native-materializer.sh "$staging/native-materializer.sh" || return 1
  jain_copy_control_plane_file "$control_root" "$control_commit" \
    ops/ci/native-runtime.sh "$staging/native-runtime.sh" || return 1
  jain_copy_control_plane_file "$control_root" "$control_commit" \
    ops/ci/split-host-ci.sh "$staging/split-host-ci.sh" || return 1
  jain_copy_control_plane_file "$control_root" "$control_commit" \
    ops/ci/host-ci-integrity.sh "$staging/host-ci-integrity.sh" || return 1
  recorded_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  jq -n --arg owner "$owner" --arg repo "$repo" --arg head_sha "$head_sha" \
    --arg required_check "$check" --arg recorded_at "$recorded_at" \
    --arg control_commit "$control_commit" \
    --arg log_sha256 "$(sha256sum -- "$staging/materialization.log" | cut -d' ' -f1)" \
    --arg manifest_sha256 "$(sha256sum -- "$staging/native-vendor-manifest.json" | cut -d' ' -f1)" \
    --arg authority_sha256 "$(sha256sum -- "$staging/native-sources.lock.json" | cut -d' ' -f1)" \
    --arg materializer_sha256 "$(sha256sum -- "$staging/native-materializer.sh" | cut -d' ' -f1)" \
    --arg native_runtime_sha256 "$(sha256sum -- "$staging/native-runtime.sh" | cut -d' ' -f1)" \
    --arg split_host_ci_sha256 "$(sha256sum -- "$staging/split-host-ci.sh" | cut -d' ' -f1)" \
    --arg host_ci_integrity_sha256 "$(sha256sum -- "$staging/host-ci-integrity.sh" | cut -d' ' -f1)" \
    --slurpfile manifest "$staging/native-vendor-manifest.json" \
    '{schema_version:"jain.host-native-materialization/v1",recorded_at:$recorded_at,
      owner:$owner,repository:$repo,head_sha:$head_sha,required_check:$required_check,
      status:"pass",control_plane_commit:$control_commit,
      authority:{file:"native-sources.lock.json",sha256:$authority_sha256},
      materializer:{file:"native-materializer.sh",sha256:$materializer_sha256},
      orchestration:{native_runtime_file:"native-runtime.sh",
        native_runtime_sha256:$native_runtime_sha256,
        split_host_ci_file:"split-host-ci.sh",split_host_ci_sha256:$split_host_ci_sha256,
        host_ci_integrity_file:"host-ci-integrity.sh",
        host_ci_integrity_sha256:$host_ci_integrity_sha256},
      log_file:"materialization.log",log_sha256:$log_sha256,
      native_vendor_manifest_file:"native-vendor-manifest.json",
      native_vendor_manifest_sha256:$manifest_sha256,
      native_vendor_manifest:$manifest[0]}' >"$staging/receipt.json"
  for file in materialization.log native-vendor-manifest.json \
    native-sources.lock.json native-materializer.sh native-runtime.sh \
    split-host-ci.sh host-ci-integrity.sh receipt.json; do
    jain_write_sha256_sidecar "$staging/$file" || {
      rm -rf -- "$staging"
      return 1
    }
  done
  while IFS= read -r file; do
    chmod 0600 -- "$staging/$file" || {
      rm -rf -- "$staging"
      return 1
    }
  done < <(jain_native_evidence_files)
  jain_verify_native_evidence "$staging" "$head_sha" "$check" \
    "$control_root" "$control_commit" || {
    rm -rf -- "$staging"
    return 1
  }
  mv -- "$staging" "$destination"
  jain_verify_native_evidence "$destination" "$head_sha" "$check" \
    "$control_root" "$control_commit" || return 1
  JAIN_NATIVE_EVIDENCE_DIR="$destination"
  JAIN_NATIVE_EVIDENCE_SHA256="$(sha256sum -- "$destination/receipt.json" | cut -d' ' -f1)"
  jain_verify_native_evidence_binding "$destination" "$JAIN_NATIVE_EVIDENCE_SHA256" \
    "$head_sha" "$check" "$control_root" "$control_commit" || return 1
  export JAIN_NATIVE_EVIDENCE_DIR JAIN_NATIVE_EVIDENCE_SHA256
}

jain_prepare_native_runtime() {
  local vendor_root="${1:?native vendor root is required}"
  local learner dir
  local -a build_dirs=()

  case "$vendor_root" in
    /*) ;;
    *)
      printf 'native runtime root must be absolute: %s\n' "$vendor_root" >&2
      return 1
      ;;
  esac
  [[ -d "$vendor_root" ]] || {
    printf 'native runtime root does not exist: %s\n' "$vendor_root" >&2
    return 1
  }
  for learner in catboost xgboost lightgbm; do
    [[ -d "$vendor_root/$learner" ]] || {
      printf 'native source directory does not exist: %s\n' "$vendor_root/$learner" >&2
      return 1
    }
  done

  # These are build destinations, not evidence that a learner built. In
  # particular, never create CatBoost's nested final library directory here:
  # only the CatBoost build is allowed to produce it.
  mapfile -t build_dirs < <(jain_native_build_dirs "$vendor_root")
  mkdir -p "${build_dirs[@]}"
  for dir in "${build_dirs[@]}"; do
    [[ -d "$dir" ]] || {
      printf 'native build destination is unavailable: %s\n' "$dir" >&2
      return 1
    }
  done

  export JAIN_VENDOR_ROOT="$vendor_root"
  export CATBOOST_BUILD_DIR="${build_dirs[0]}"
  export XGBOOST_BUILD_DIR="${build_dirs[1]}"
  export LIGHTGBM_BUILD_DIR="${build_dirs[2]}"
  jain_export_native_runtime_path "$vendor_root"
}

jain_export_native_runtime_path() {
  local vendor_root="${1:?native vendor root is required}"
  local -a runtime_dirs=()

  case "$vendor_root" in
    /*) ;;
    *)
      printf 'native runtime root must be absolute: %s\n' "$vendor_root" >&2
      return 1
      ;;
  esac
  [[ -d "$vendor_root" ]] || {
    printf 'native runtime root does not exist: %s\n' "$vendor_root" >&2
    return 1
  }

  mapfile -t runtime_dirs < <(jain_native_runtime_dirs "$vendor_root")

  local joined
  joined="$(IFS=:; printf '%s' "${runtime_dirs[*]}")"
  if [[ -n "${LD_LIBRARY_PATH:-}" ]]; then
    joined="$joined:$LD_LIBRARY_PATH"
  fi
  export LD_LIBRARY_PATH="$joined"
}

jain_verify_elf_dependencies() {
  local artifact="${1:?ELF artifact is required}"
  local description output

  [[ -f "$artifact" && -s "$artifact" ]] || {
    printf 'native artifact is missing or empty: %s\n' "$artifact" >&2
    return 1
  }
  command -v file >/dev/null 2>&1 || {
    printf 'required native verification tool is unavailable: file\n' >&2
    return 1
  }
  command -v ldd >/dev/null 2>&1 || {
    printf 'required native verification tool is unavailable: ldd\n' >&2
    return 1
  }
  description="$(file -b -- "$artifact")" || return 1
  case "$description" in
    ELF*"dynamically linked"* | ELF*"shared object"*) ;;
    *)
      printf 'native artifact is not a dynamically linked ELF: %s (%s)\n' \
        "$artifact" "$description" >&2
      return 1
      ;;
  esac
  if ! output="$(ldd "$artifact" 2>&1)"; then
    printf 'ldd failed for native artifact %s:\n%s\n' "$artifact" "$output" >&2
    return 1
  fi
  if grep -Fq 'not found' <<<"$output"; then
    printf 'unresolved runtime dependency for %s:\n%s\n' "$artifact" "$output" >&2
    return 1
  fi
}

jain_verify_native_libraries() {
  local vendor_root="${1:?native vendor root is required}"
  shift
  local learner library
  local -a learners=("$@")
  if [[ "${#learners[@]}" -eq 0 ]]; then
    learners=(catboost xgboost lightgbm)
  fi
  for learner in "${learners[@]}"; do
    library="$(jain_native_library_path "$vendor_root" "$learner")" || return 1
    [[ -f "$library" && -s "$library" ]] || {
      printf '%s release build did not produce exact non-empty library: %s\n' \
        "$learner" "$library" >&2
      return 1
    }
    jain_verify_elf_dependencies "$library" || return 1
  done
}

jain_verify_linked_binaries() {
  local release_dir="${1:?Cargo release directory is required}"
  local artifact description output
  local checked=0

  [[ -d "$release_dir" ]] || {
    printf 'Cargo release directory does not exist: %s\n' "$release_dir" >&2
    return 1
  }
  while IFS= read -r -d '' artifact; do
    description="$(file -b -- "$artifact")" || return 1
    case "$description" in
      ELF*"dynamically linked"* | ELF*"shared object"*) ;;
      *) continue ;;
    esac
    if ! output="$(ldd "$artifact" 2>&1)"; then
      printf 'ldd failed for linked release artifact %s:\n%s\n' \
        "$artifact" "$output" >&2
      return 1
    fi
    if grep -Fq 'not found' <<<"$output"; then
      printf 'unresolved runtime dependency for %s:\n%s\n' \
        "$artifact" "$output" >&2
      return 1
    fi
    checked=$((checked + 1))
  done < <(find "$release_dir" -type f \
    \( -perm /111 -o -name '*.so' -o -name '*.so.*' \) -print0)
  [[ "$checked" -gt 0 ]] || {
    printf 'native release build produced no linked ELF artifacts under %s\n' \
      "$release_dir" >&2
    return 1
  }
}
