#!/usr/bin/env bash
# Root-only exact-SHA Jankurai evidence promotion and verification helpers.

JAIN_HOST_CI_JANKURAI_VERSION='jankurai 1.6.11'
JAIN_PROOF_STAGING_MAX_FILES=3
JAIN_PROOF_STAGING_MAX_BYTES=16777216
JAIN_PROOF_STAGING_MAX_FILE_BYTES=8388608
JAIN_PROOF_EVIDENCE_MIN_FREE_BYTES=1073741824
JAIN_PROOF_EVIDENCE_RETAIN_PER_CHECK=8
readonly JAIN_HOST_CI_JANKURAI_VERSION
readonly JAIN_PROOF_STAGING_MAX_FILES JAIN_PROOF_STAGING_MAX_BYTES
readonly JAIN_PROOF_STAGING_MAX_FILE_BYTES JAIN_PROOF_EVIDENCE_MIN_FREE_BYTES
readonly JAIN_PROOF_EVIDENCE_RETAIN_PER_CHECK

jain_host_ci_verify_proof_store() {
  local store="${1:?proof evidence store is required}" resolved
  resolved="$(realpath -e -- "$store")" || return 1
  case "$resolved" in
    /tmp | /tmp/*) return 1 ;;
  esac
  [[ "$resolved" == "$store" && -d "$resolved" && ! -L "$resolved" \
    && "$(stat -c '%u:%g:%a' -- "$resolved")" == '0:0:700' ]] || return 1
  printf '%s\n' "$resolved"
}

jain_host_ci_verify_proof_staging() {
  local staging="${1:?proof staging root is required}"
  local worker_uid="${2:?worker UID is required}"
  local worker_gid="${3:?worker GID is required}"
  local resolved file mode size total=0
  local -a expected=(repair-queue.jsonl report.json report.md) actual=()
  resolved="$(realpath -e -- "$staging")" || return 1
  [[ "$resolved" == "$staging" && -d "$resolved" && ! -L "$resolved" \
    && "$(stat -f -c '%T' -- "$resolved")" == tmpfs \
    && "$(stat -c '%u:%g:%a' -- "$resolved")" \
      == "$worker_uid:$worker_gid:700" ]] || return 1
  mapfile -t actual < <(
    find "$resolved" -xdev -mindepth 1 -maxdepth 1 -printf '%f\n' \
      | LC_ALL=C sort
  )
  [[ "${#actual[@]}" == "$JAIN_PROOF_STAGING_MAX_FILES" \
    && "${actual[*]}" == "${expected[*]}" ]] || return 1
  for file in "${expected[@]}"; do
    [[ -f "$resolved/$file" && ! -L "$resolved/$file" \
      && "$(stat -c '%u:%g:%h' -- "$resolved/$file")" \
        == "$worker_uid:$worker_gid:1" ]] || return 1
    mode="$(stat -c '%a' -- "$resolved/$file")" || return 1
    (( (8#$mode & 8#022) == 0 )) || return 1
    size="$(stat -c '%s' -- "$resolved/$file")" || return 1
    (( size > 0 && size <= JAIN_PROOF_STAGING_MAX_FILE_BYTES )) || return 1
    total=$((total + size))
    (( total <= JAIN_PROOF_STAGING_MAX_BYTES )) || return 1
  done
  jq -e 'type == "object"' "$resolved/report.json" >/dev/null || return 1
  printf '%s\n' "$resolved"
}

jain_host_ci_verify_proof_payload() {
  local evidence_dir="${1:?proof evidence directory is required}"
  local owner="${2:?owner is required}" repo="${3:?repository is required}"
  local head="${4:?head SHA is required}" check="${5:?check is required}"
  local attempt_id="${6:?attempt ID is required}"
  local auditor_sha="${7:?auditor digest is required}"
  local expected_status="${8:?proof status is required}"
  local report="$evidence_dir/report.json" receipt="$evidence_dir/receipt.json"
  local actual_report_sha
  [[ "$owner" =~ ^[a-z0-9][a-z0-9-]*$ \
    && "$repo" =~ ^[a-z0-9][a-z0-9-]*$ \
    && "$head" =~ ^[0-9a-f]{40}$ \
    && "$check" =~ ^[a-z0-9][a-z0-9-]*/required$ \
    && "$attempt_id" =~ ^[A-Za-z0-9_.-]+$ \
    && "$auditor_sha" =~ ^[0-9a-f]{64}$ \
    && "$expected_status" =~ ^(pass|fail)$ ]] || return 1
  for file in "$report" "$receipt"; do
    [[ -f "$file" && ! -L "$file" \
      && "$(stat -c '%h' -- "$file" 2>/dev/null)" == 1 ]] || return 1
  done
  actual_report_sha="$(sha256sum -- "$report" | cut -d' ' -f1)" || return 1
  jq -e --arg owner "$owner" --arg repo "$repo" --arg head "$head" \
    --arg check "$check" --arg attempt "$attempt_id" \
    --arg auditor_sha "$auditor_sha" \
    --arg auditor_version "$JAIN_HOST_CI_JANKURAI_VERSION" \
    --arg report "$report" --arg report_sha "$actual_report_sha" \
    --arg status "$expected_status" '
      select(.schema_version == "jain.jankurai-exact-sha-evidence/v1")
      | select(.operation == "jankurai-evidence" and .mode == "evidence")
      | select(.status == $status)
      | select(.repository == $repo and .commit == $head)
      | select(.attempt_id == $attempt)
      | select(.run_id | type == "string" and length > 0)
      | select(.report == $report and .report_sha256 == $report_sha)
      | select(.policy.sha256 | test("^[0-9a-f]{64}$"))
      | select(.baseline.configured | type == "boolean")
      | select(.baseline.mode == (if .baseline.configured
          then "governed-baseline" else "policy-floor-only" end))
      | select(if .baseline.configured
          then ((.baseline.sha256 | test("^[0-9a-f]{64}$"))
            and (.baseline.score | type == "number")
            and (.baseline.auditor | type == "string" and length > 0))
          else (.baseline.sha256 == null and .baseline.score == null
            and .baseline.auditor == null)
        end)
      | select(.auditor.version == $auditor_version)
      | select(.auditor.sha256 == $auditor_sha)
      | select(.score | type == "number")
      | select(.hard_findings | type == "number")
      | select(.caps_applied | type == "number")
      | select(.clean_tracked_tree_at_start == true)
      | select(.clean_tracked_tree_at_finish == true)
      | select(.authority_failures == [])
      | select(.gate_failures | type == "array")
      | select(.failures == (.authority_failures + .gate_failures))
      | select(if $status == "pass"
          then (.lane.conclusion == "success" and .gate_failures == []
            and .failures == [] and .hard_findings == 0
            and .caps_applied == 0 and .ratchet_passed == true
            and .report_identity.decision_passed == true
            and .report_identity.conformance_decision == "pass"
            and .report_identity.conformance_blockers == [])
          else (.gate_failures | length > 0)
        end)' "$receipt" >/dev/null || return 1
}

# Revalidates deploy release-tag authority in both root processes that consume
# promoted proof evidence. During promotion, root_request is the sandbox-owned
# request root. During publication, request_dir is the protected publisher's
# view of that same immutable request. Exactly one context must exist.
jain_host_ci_verify_deploy_source_tags() (
  local owner="${1:?owner is required}" repo="${2:?repository is required}"
  local head="${3:?head SHA is required}" check="${4:?check is required}"
  local sandbox_root_present=0 publisher_root_present=0 authority_root
  local deploy_lock deploy_lock_sha deploy_lock_blob deploy_lock_authority
  local sibling_row sibling sibling_owner sibling_remote sibling_commit
  local sibling_stage
  local release_status release_ref release_commit contract_ref contract_object
  local contract_commit retained_tags expected_tags
  local -a expected_safe_git=(git -c core.fsmonitor=false \
    -c core.hooksPath=/dev/null -c core.untrackedCache=false \
    -c diff.external=)

  jain_host_ci_deploy_source_tags_fail() {
    printf '[host-ci-proof-evidence] %s\n' "$*" >&2
    exit 1
  }

  [[ "$repo" == jain-deploy ]] || return 0
  [[ "$(id -u)" == 0 \
    && "$owner" =~ ^[a-z0-9][a-z0-9-]*$ \
    && "$head" =~ ^[0-9a-f]{40}$ \
    && "$check" == jain-deploy/required ]] \
    || jain_host_ci_deploy_source_tags_fail \
      'deploy proof identity is malformed'

  [[ ${root_request+x} ]] && sandbox_root_present=1
  [[ ${request_dir+x} ]] && publisher_root_present=1
  (( sandbox_root_present + publisher_root_present == 1 )) \
    || jain_host_ci_deploy_source_tags_fail \
      'deploy proof requires exactly one root request authority'
  if (( sandbox_root_present == 1 )); then
    authority_root="$root_request"
  else
    authority_root="$request_dir"
  fi

  for required_global in product_authority sibling_sources_path request_id \
    control_commit splitctl_path token_file family_root; do
    [[ ${!required_global+x} && -n "${!required_global}" ]] \
      || jain_host_ci_deploy_source_tags_fail \
        "deploy proof lacks required authority: $required_global"
  done
  declare -p safe_git >/dev/null 2>&1 \
    || jain_host_ci_deploy_source_tags_fail \
      'deploy proof lacks the reviewed Git command'
  [[ "$(declare -p safe_git)" == 'declare -a safe_git='* \
    && "${#safe_git[@]}" == "${#expected_safe_git[@]}" \
    && "${safe_git[*]}" == "${expected_safe_git[*]}" ]] \
    || jain_host_ci_deploy_source_tags_fail \
      'deploy proof Git command differs from reviewed authority'

  [[ "$authority_root" == /* && -d "$authority_root" \
    && ! -L "$authority_root" \
    && "$product_authority" == "$authority_root/product-authority" \
    && "$sibling_sources_path" \
      == "$authority_root/worker-authority/sibling-sources.json" \
    && "$request_id" =~ ^[0-9a-f]{64}$ \
    && "$control_commit" =~ ^[0-9a-f]{40}$ \
    && "$family_root" == /* && -d "$family_root" && ! -L "$family_root" \
    && -f "$splitctl_path" && ! -L "$splitctl_path" \
    && "$(stat -c '%u:%g:%a:%h' -- "$splitctl_path" 2>/dev/null)" \
      == '0:0:500:1' \
    && -f "$token_file" && ! -L "$token_file" \
    && "$(stat -c '%u:%g:%a:%h' -- "$token_file" 2>/dev/null)" \
      == '0:0:600:1' ]] \
    || jain_host_ci_deploy_source_tags_fail \
      'deploy proof paths or root identities are not exact authority'

  [[ -d "$product_authority/.git" && ! -L "$product_authority" \
    && ! -L "$product_authority/.git" \
    && -f "$sibling_sources_path" && ! -L "$sibling_sources_path" \
    && "$(stat -c '%u:%g:%a:%h' -- "$sibling_sources_path" 2>/dev/null)" \
      == '0:0:444:1' ]] \
    || jain_host_ci_deploy_source_tags_fail \
      'deploy proof source inventory is not immutable root authority'

  jq -e --arg request_id "$request_id" --arg control "$control_commit" \
    --arg owner "$owner" --arg head "$head" --arg check "$check" \
    --arg family_root "$family_root" '
    select((keys | sort) == ([
      "control_plane_commit","deploy_source_lock_sha256","head_sha","owner",
      "reference","repository","request_id","required_check",
      "schema_version","sources"
    ] | sort))
    | select(.schema_version == "jain.host-ci-sibling-sources/v1")
    | select(.request_id == $request_id and .control_plane_commit == $control)
    | select(.owner == $owner and .repository == "jain-deploy")
    | select(.head_sha == $head and .required_check == $check)
    | select(.reference == "refs/heads/main")
    | select(.deploy_source_lock_sha256 | test("^[0-9a-f]{64}$"))
    | select((.sources | type) == "array" and (.sources | length) > 0)
    | select(.sources == (.sources | sort_by(.repository)))
    | select((.sources | map(.repository) | unique | length)
        == (.sources | length))
    | select(all(.sources[];
        (keys | sort) == ([
          "commit","contract_tag_commit","contract_tag_object",
          "contract_tag_ref","entry_count","inventory_sha256","mount_path",
          "owner","reference","release_tag_commit","release_tag_ref",
          "release_tag_status","remote","repository","tree"
        ] | sort)
        and (.repository | test("^[a-z0-9][a-z0-9-]*$"))
        and (
          (
            .repository == "redline-split-ops"
            and (.owner == "jeryu" or .owner == "veox")
            and .remote == ("http://127.0.0.1:8787/git/" + .owner
              + "/redline-split-ops.git")
            and .mount_path
              == ($family_root + "/jain-redline/redline-split-ops")
          )
          or
          (
            .repository != "redline-split-ops"
            and .owner == "veox"
            and .remote == ("http://127.0.0.1:8787/git/veox/"
              + .repository + ".git")
            and .mount_path == ($family_root + "/" + .repository)
          )
        )
        and .reference == "refs/heads/main"
        and (.commit | test("^[0-9a-f]{40}$"))
        and (.tree | test("^[0-9a-f]{40}$"))
        and (.inventory_sha256 | test("^[0-9a-f]{64}$"))
        and (.entry_count | type) == "number" and .entry_count >= 0
        and (
          (
            (.release_tag_status == "pending"
              or .release_tag_status == "absent")
            and .release_tag_ref == ""
            and .release_tag_commit == ""
          )
          or
          (
            .release_tag_status == "bound"
            and (.repository as $source
              | .release_tag_ref
              | startswith("refs/tags/" + $source + "-v"))
            and (.release_tag_ref
              | test("-v[0-9A-Za-z.-]+-split\\.[0-9]+$"))
            and (.release_tag_commit | test("^[0-9a-f]{40}$"))
          )
        )
        and (
          (
            .contract_tag_ref == ""
            and .contract_tag_object == ""
            and .contract_tag_commit == ""
          )
          or
          (
            .repository == "jain-core"
            and (.contract_tag_ref
              | test("^refs/tags/jain-core-v[0-9A-Za-z.-]+-split\\.[0-9]+$"))
            and (.contract_tag_object | test("^[0-9a-f]{40}$"))
            and (.contract_tag_commit | test("^[0-9a-f]{40}$"))
          )
        )
      ))' "$sibling_sources_path" >/dev/null \
    || jain_host_ci_deploy_source_tags_fail \
      'deploy proof source inventory has an invalid closed shape'

  deploy_lock="$product_authority/jain-split.lock.toml"
  deploy_lock_sha="$(jq -er '.deploy_source_lock_sha256' \
    "$sibling_sources_path")" \
    || jain_host_ci_deploy_source_tags_fail \
      'deploy proof source inventory lacks its lock digest'
  [[ -f "$deploy_lock" && ! -L "$deploy_lock" \
    && "$(stat -c '%u:%g:%a:%h' -- "$deploy_lock" 2>/dev/null)" \
      == '0:0:644:1' \
    && "$(stat -c '%s' -- "$deploy_lock")" -le 1048576 \
    && "$(sha256sum -- "$deploy_lock" | cut -d' ' -f1)" \
      == "$deploy_lock_sha" ]] \
    || jain_host_ci_deploy_source_tags_fail \
      'deploy proof source lock is missing or changed'
  deploy_lock_blob="$("${safe_git[@]}" -C "$product_authority" \
    rev-parse --verify "$head:jain-split.lock.toml")" \
    || jain_host_ci_deploy_source_tags_fail \
      'deploy proof source lock is not tracked at the exact head'
  deploy_lock_authority="$("$splitctl_path" host-ci-deploy-source-lock \
    --lock "$deploy_lock")" \
    || jain_host_ci_deploy_source_tags_fail \
      'deploy proof cannot reparse its source lock'
  [[ "$deploy_lock_blob" =~ ^[0-9a-f]{40}$ \
    && "$("${safe_git[@]}" -C "$product_authority" \
      hash-object --no-filters -- "$deploy_lock")" == "$deploy_lock_blob" \
    && "$(jq -er '.lock_sha256' <<<"$deploy_lock_authority")" \
      == "$deploy_lock_sha" ]] \
    || jain_host_ci_deploy_source_tags_fail \
      'deploy proof source lock differs from product authority'
  jq -e --argjson lock_authority "$deploy_lock_authority" '
    (
      [.sources[] | {
        repository,
        status:.release_tag_status,
        release_tag_ref,
        release_tag_commit
      }]
      | sort_by(.repository)
    ) == (
      (
        [$lock_authority.sources[]
          | select(.repository != "jain-deploy")]
        + [{
          repository:"jain-shard",
          status:"absent",
          release_tag_ref:"",
          release_tag_commit:""
        }]
      )
      | sort_by(.repository)
    )' "$sibling_sources_path" >/dev/null \
    || jain_host_ci_deploy_source_tags_fail \
      'deploy proof release-tag tuples differ from the tracked lock'

  while IFS= read -r sibling_row; do
    sibling="$(jq -er '.repository' <<<"$sibling_row")" \
      || jain_host_ci_deploy_source_tags_fail \
        'deploy proof cannot read a sibling repository'
    sibling_owner="$(jq -er '.owner' <<<"$sibling_row")" \
      || jain_host_ci_deploy_source_tags_fail \
        "deploy proof cannot read sibling owner: $sibling"
    sibling_remote="$(jq -er '.remote' <<<"$sibling_row")" \
      || jain_host_ci_deploy_source_tags_fail \
        "deploy proof cannot read sibling remote: $sibling"
    sibling_commit="$(jq -er '.commit' <<<"$sibling_row")" \
      || jain_host_ci_deploy_source_tags_fail \
        "deploy proof cannot read sibling commit: $sibling"
    release_status="$(jq -er '.release_tag_status' <<<"$sibling_row")" \
      || jain_host_ci_deploy_source_tags_fail \
        "deploy proof cannot read sibling release status: $sibling"
    release_ref="$(jq -er '.release_tag_ref' <<<"$sibling_row")" \
      || jain_host_ci_deploy_source_tags_fail \
        "deploy proof cannot read sibling release ref: $sibling"
    release_commit="$(jq -er '.release_tag_commit' <<<"$sibling_row")" \
      || jain_host_ci_deploy_source_tags_fail \
        "deploy proof cannot read sibling release commit: $sibling"
    contract_ref="$(jq -er '.contract_tag_ref' <<<"$sibling_row")" \
      || jain_host_ci_deploy_source_tags_fail \
        "deploy proof cannot read sibling contract ref: $sibling"
    contract_object="$(jq -er '.contract_tag_object' <<<"$sibling_row")" \
      || jain_host_ci_deploy_source_tags_fail \
        "deploy proof cannot read sibling contract object: $sibling"
    contract_commit="$(jq -er '.contract_tag_commit' <<<"$sibling_row")" \
      || jain_host_ci_deploy_source_tags_fail \
        "deploy proof cannot read sibling contract commit: $sibling"
    sibling_stage="$authority_root/worker-authority/sibling-checkouts/$sibling"
    [[ -d "$sibling_stage/.git" && ! -L "$sibling_stage" \
      && ! -L "$sibling_stage/.git" \
      && "$("${safe_git[@]}" -C "$sibling_stage" \
        rev-parse --verify 'HEAD^{commit}')" == "$sibling_commit" \
      && -z "$("${safe_git[@]}" -C "$sibling_stage" remote)" \
      && -z "$("${safe_git[@]}" -C "$sibling_stage" \
        status --porcelain=v1 --untracked-files=all)" ]] \
      || jain_host_ci_deploy_source_tags_fail \
        "deploy proof sibling checkout changed: $sibling"
    retained_tags="$("${safe_git[@]}" -C "$sibling_stage" \
      for-each-ref --format='%(refname)' refs/tags)" \
      || jain_host_ci_deploy_source_tags_fail \
        "deploy proof cannot enumerate sibling tags: $sibling"
    if [[ "$release_status" == bound ]]; then
      [[ "$release_ref" \
          =~ ^refs/tags/${sibling}-v[0-9A-Za-z.-]+-split\.[0-9]+$ \
        && "$release_commit" =~ ^[0-9a-f]{40}$ \
        && "$("${safe_git[@]}" -C "$sibling_stage" rev-parse --verify \
          "$release_ref")" == "$release_commit" \
        && "$("${safe_git[@]}" -C "$sibling_stage" rev-parse --verify \
          "$release_ref^{commit}")" == "$release_commit" ]] \
        && "${safe_git[@]}" -C "$sibling_stage" merge-base --is-ancestor \
          "$release_commit" "$sibling_commit" \
        || jain_host_ci_deploy_source_tags_fail \
          "deploy proof release tag changed: $sibling"
    else
      [[ "$release_status" == pending || "$release_status" == absent ]] \
        && [[ -z "$release_ref" && -z "$release_commit" ]] \
        || jain_host_ci_deploy_source_tags_fail \
          "deploy proof unbound release tuple changed: $sibling"
    fi
    if [[ -n "$contract_ref" ]]; then
      [[ "$sibling" == jain-core \
        && "$contract_ref" \
          =~ ^refs/tags/jain-core-v[0-9A-Za-z.-]+-split\.[0-9]+$ \
        && "$contract_object" =~ ^[0-9a-f]{40}$ \
        && "$contract_commit" =~ ^[0-9a-f]{40}$ \
        && "$("${safe_git[@]}" -C "$sibling_stage" rev-parse --verify \
          "$contract_ref")" == "$contract_object" \
        && "$("${safe_git[@]}" -C "$sibling_stage" cat-file -t \
          "$contract_object")" == tag \
        && "$("${safe_git[@]}" -C "$sibling_stage" rev-parse --verify \
          "$contract_ref^{commit}")" == "$contract_commit" ]] \
        && "${safe_git[@]}" -C "$sibling_stage" merge-base --is-ancestor \
          "$contract_commit" "$sibling_commit" \
        || jain_host_ci_deploy_source_tags_fail \
          'deploy proof Core contract tag changed'
    else
      [[ -z "$contract_object" && -z "$contract_commit" ]] \
        || jain_host_ci_deploy_source_tags_fail \
          'deploy proof carried a partial Core contract tag'
    fi
    expected_tags="$({
      printf '%s\n' "$release_ref" "$contract_ref"
    } | sed '/^$/d' | LC_ALL=C sort -u)"
    [[ "$retained_tags" == "$expected_tags" ]] \
      || jain_host_ci_deploy_source_tags_fail \
        "deploy proof sibling retained a missing or extra tag: $sibling"
    "$splitctl_path" jeryu-local ref-readback \
      --repo "$sibling_owner/$sibling" --remote "$sibling_remote" \
      --ref refs/heads/main --expected-head "$sibling_commit" \
      --token-file "$token_file" >/dev/null \
      || jain_host_ci_deploy_source_tags_fail \
        "deploy proof sibling protected main moved: $sibling"
    if [[ -n "$release_ref" ]]; then
      "$splitctl_path" jeryu-local ref-readback \
        --repo "$sibling_owner/$sibling" --remote "$sibling_remote" \
        --ref "$release_ref" --expected-head "$release_commit" \
        --token-file "$token_file" >/dev/null \
        || jain_host_ci_deploy_source_tags_fail \
          "deploy proof sibling release tag moved: $sibling"
    fi
  done < <(jq -c '.sources[]' "$sibling_sources_path")
)

jain_host_ci_verify_promoted_proof_evidence() {
  local evidence_dir="${1:?proof evidence directory is required}"
  shift
  local owner="${1:?owner is required}" repo="${2:?repository is required}"
  local head="${3:?head SHA is required}" check="${4:?check is required}"
  local file
  [[ -d "$evidence_dir" && ! -L "$evidence_dir" \
    && "$(stat -c '%u:%g:%a' -- "$evidence_dir")" == '0:0:500' ]] \
    || return 1
  [[ "$(find "$evidence_dir" -mindepth 1 -maxdepth 1 -printf '%f\n' \
    | LC_ALL=C sort | tr '\n' ' ')" == 'receipt.json report.json ' ]] || return 1
  for file in receipt.json report.json; do
    [[ "$(stat -c '%u:%g:%a:%h' -- "$evidence_dir/$file")" \
      == '0:0:400:1' ]] || return 1
  done
  jain_host_ci_verify_proof_payload "$evidence_dir" "$@" || return 1
  jain_host_ci_verify_deploy_source_tags "$owner" "$repo" "$head" "$check"
}

jain_host_ci_ensure_proof_store_dir() {
  local parent="${1:?parent directory is required}"
  local component="${2:?directory component is required}" next
  [[ "$component" =~ ^[A-Za-z0-9_.-]+$ \
    && "$component" != . && "$component" != .. ]] || return 1
  next="$parent/$component"
  if [[ -e "$next" || -L "$next" ]]; then
    [[ -d "$next" && ! -L "$next" \
      && "$(stat -c '%u:%g:%a' -- "$next")" == '0:0:700' ]] || return 1
  else
    install -d -o root -g root -m 0700 -- "$next" || return 1
  fi
  printf '%s\n' "$next"
}

# Prints: evidence_dir<TAB>receipt_sha<TAB>report_sha<TAB>status<TAB>validator_rc
jain_host_ci_promote_proof_evidence() (
  local staging="${1:?proof staging root is required}"
  local store="${2:?proof evidence store is required}"
  local owner="${3:?owner is required}" repo="${4:?repository is required}"
  local head="${5:?head SHA is required}" check="${6:?check is required}"
  local request_id="${7:?request ID is required}"
  local attempt_id="${8:?attempt ID is required}"
  local worker_uid="${9:?worker UID is required}"
  local worker_gid="${10:?worker GID is required}"
  local worktree="${11:?audit worktree is required}"
  local splitctl="${12:?splitctl is required}" auditor="${13:?auditor is required}"
  local expected_auditor_sha="${14:?auditor digest is required}"
  local lane_conclusion="${15:?lane conclusion is required}"
  local lane_failure_reason="${16:-}" clean_start="${17:?clean start is required}"
  local resolved_store resolved_staging parent destination check_slug available required
  local validator_rc=0 status receipt_sha report_sha auditor_sha lock lock_fd name index
  local -a retained=() failure_args=()
  [[ "$(id -u)" == 0 && "$request_id" =~ ^[0-9a-f]{64}$ \
    && "$attempt_id" =~ ^[A-Za-z0-9_.-]+$ \
    && "$expected_auditor_sha" =~ ^[0-9a-f]{64}$ \
    && "$lane_conclusion" =~ ^(success|failure)$ \
    && "$clean_start" =~ ^(true|false)$ ]] || return 1
  [[ "$lane_conclusion" == success || -n "$lane_failure_reason" ]] || return 1
  resolved_store="$(jain_host_ci_verify_proof_store "$store")" || return 1
  resolved_staging="$(jain_host_ci_verify_proof_staging \
    "$staging" "$worker_uid" "$worker_gid")" || return 1
  auditor_sha="$(sha256sum -- "$auditor" | cut -d' ' -f1)" || return 1
  [[ "$auditor_sha" == "$expected_auditor_sha" \
    && "$("$auditor" --version)" == "$JAIN_HOST_CI_JANKURAI_VERSION" ]] \
    || return 1

  lock="$resolved_store/.promotion.lock"
  exec {lock_fd}>"$lock" || return 1
  chmod 0600 "$lock" && chown root:root "$lock" || return 1
  /usr/bin/flock -x "$lock_fd" || return 1
  available="$(df -B1 --output=avail "$resolved_store" | tail -n 1 | tr -d ' ')" \
    || return 1
  required=$(( $(stat -c '%s' -- "$resolved_staging/report.json") \
    + JAIN_PROOF_EVIDENCE_MIN_FREE_BYTES ))
  [[ "$available" =~ ^[0-9]+$ ]] && (( available >= required )) || return 1

  check_slug="${check//[^A-Za-z0-9_.-]/_}"
  parent="$resolved_store"
  for name in "$owner" "$repo" "$check_slug"; do
    parent="$(jain_host_ci_ensure_proof_store_dir "$parent" "$name")" || return 1
  done
  destination="$parent/$request_id"
  [[ ! -e "$destination" && ! -L "$destination" ]] || return 1
  install -d -o root -g root -m 0700 -- "$destination" || return 1
  if ! install -o root -g root -m 0600 -- \
      "$resolved_staging/report.json" "$destination/report.json"; then
    rm -rf -- "$destination"
    return 1
  fi
  if [[ "$lane_conclusion" == failure ]]; then
    failure_args=(--lane-failure-reason "$lane_failure_reason")
  fi
  "$splitctl" jankurai-evidence \
    --repository "$repo" --commit "$head" --worktree "$worktree" \
    --report-root "$destination" --report "$destination/report.json" \
    --auditor "$auditor" --attempt-id "$attempt_id" \
    --lane-conclusion "$lane_conclusion" "${failure_args[@]}" \
    --clean-tracked-tree-start "$clean_start" \
    --receipt "$destination/receipt.json" >&2 || validator_rc=$?
  if [[ ! -f "$destination/receipt.json" \
    || -L "$destination/receipt.json" \
    || "$(stat -c '%h' -- "$destination/receipt.json" 2>/dev/null)" != 1 ]]; then
    rm -rf -- "$destination"
    return 1
  fi
  status="$(jq -er '.status | select(. == "pass" or . == "fail")' \
    "$destination/receipt.json")" || {
    rm -rf -- "$destination"
    return 1
  }
  [[ ( "$validator_rc" == 0 && "$status" == pass ) \
    || ( "$validator_rc" != 0 && "$status" == fail ) ]] || {
    rm -rf -- "$destination"
    return 1
  }
  jain_host_ci_verify_proof_payload "$destination" "$owner" "$repo" "$head" \
    "$check" "$attempt_id" "$auditor_sha" "$status" || {
    rm -rf -- "$destination"
    return 1
  }
  chmod 0400 "$destination/report.json" "$destination/receipt.json"
  chown root:root "$destination/report.json" "$destination/receipt.json"
  chmod 0500 "$destination"
  jain_host_ci_verify_promoted_proof_evidence "$destination" "$owner" "$repo" \
    "$head" "$check" "$attempt_id" "$auditor_sha" "$status" || return 1
  receipt_sha="$(sha256sum -- "$destination/receipt.json" | cut -d' ' -f1)"
  report_sha="$(sha256sum -- "$destination/report.json" | cut -d' ' -f1)"

  mapfile -t retained < <(
    find "$parent" -mindepth 1 -maxdepth 1 -type d \
      -regextype posix-extended -regex ".*/[0-9a-f]{64}" \
      -printf '%T@ %f\n' | LC_ALL=C sort -nr
  )
  for ((index = JAIN_PROOF_EVIDENCE_RETAIN_PER_CHECK; \
      index < ${#retained[@]}; index++)); do
    name="${retained[$index]#* }"
    [[ "$name" =~ ^[0-9a-f]{64}$ \
      && -d "$parent/$name" && ! -L "$parent/$name" \
      && "$(stat -c '%u:%g:%a' -- "$parent/$name")" == '0:0:500' ]] \
      || return 1
    chmod 0700 "$parent/$name"
    rm -rf -- "${parent:?}/$name"
  done
  exec {lock_fd}>&-
  printf '%s\t%s\t%s\t%s\t%s\n' \
    "$destination" "$receipt_sha" "$report_sha" "$status" "$validator_rc"
)
