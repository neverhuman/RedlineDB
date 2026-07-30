#!/usr/bin/env bash

set -euo pipefail

package_dir="${1:-}"
tarball="${2:-}"
sidecar="${3:-}"

if [[ -z "$package_dir" ]]; then
    printf 'usage: %s PACKAGE_DIR [TARBALL]\n' "$0" >&2
    exit 2
fi
if [[ ! -d "$package_dir" || -L "$package_dir" ]]; then
    printf 'release inventory: package root is not a physical directory: %s\n' "$package_dir" >&2
    exit 1
fi

bin_dir="$package_dir/bin"
expected_binary="$bin_dir/redline-testing"
if [[ ! -d "$bin_dir" || -L "$bin_dir" ]]; then
    printf 'release inventory: bin is not a physical directory\n' >&2
    exit 1
fi

mapfile -d '' bin_entries < <(find "$bin_dir" -mindepth 1 -maxdepth 1 -print0)
if [[ ${#bin_entries[@]} -ne 1 || "${bin_entries[0]}" != "$expected_binary" ]]; then
    printf 'release inventory: bin must contain exactly bin/redline-testing\n' >&2
    exit 1
fi

while IFS= read -r -d '' entry; do
    relative="${entry#"$package_dir"/}"
    [[ "$relative" =~ ^[A-Za-z0-9._/-]+$ \
        && "$relative" != /* && "$relative" != *//* \
        && "/$relative/" != *"/../"* && "/$relative/" != *"/./"* ]] || {
        printf 'release inventory: ambiguous member name: %q\n' "$relative" >&2
        exit 1
    }
    if [[ -L "$entry" ]]; then
        printf 'release inventory: symlink member is forbidden: %s\n' "$relative" >&2
        exit 1
    elif [[ -d "$entry" ]]; then
        continue
    elif [[ -f "$entry" ]]; then
        links="$(stat -c '%h' "$entry")"
        if [[ "$links" != "1" ]]; then
            printf 'release inventory: hard-linked member is forbidden: %s\n' "$relative" >&2
            exit 1
        fi
    else
        printf 'release inventory: special member is forbidden: %s\n' "$relative" >&2
        exit 1
    fi
done < <(find "$package_dir" -mindepth 1 -print0)

if [[ ! -f "$expected_binary" || -L "$expected_binary" || ! -x "$expected_binary" ]]; then
    printf 'release inventory: bin/redline-testing must be one executable regular file\n' >&2
    exit 1
fi

if [[ -n "$tarball" ]]; then
    [[ -n "$sidecar" ]] || {
        printf 'release inventory: archive verification requires a sidecar path\n' >&2
        exit 2
    }
    [[ -f "$tarball" && ! -L "$tarball" ]] || {
        printf 'release inventory: tarball is not a physical regular file\n' >&2
        exit 1
    }
    tarball="$(realpath -e -- "$tarball")"
    tarball_parent="$(dirname "$tarball")"
    [[ -d "$tarball_parent" && ! -L "$tarball_parent" ]] || {
        printf 'release inventory: tarball parent is not a physical directory\n' >&2
        exit 1
    }
    exec {archive_fd}<"$tarball"
    archive_path="/proc/$$/fd/$archive_fd"
    archive_identity="$(stat -Lc '%d:%i:%h:%s:%F' "$archive_path")"
    path_identity="$(stat -Lc '%d:%i:%h:%s:%F' "$tarball")"
    [[ "$archive_identity" == "$path_identity" \
        && "$archive_identity" == *":1:"* \
        && "$archive_identity" == *":regular file" ]] || {
        printf 'release inventory: archive descriptor custody is unsafe\n' >&2
        exit 1
    }

    package_name="$(basename "$package_dir")"
    expected_members="$(mktemp)"
    actual_members="$(mktemp)"
    cleanup_inventory() {
        rm -f -- "$expected_members" "$actual_members"
    }
    trap cleanup_inventory EXIT
    {
        printf '%s/\n' "$package_name"
        while IFS= read -r -d '' entry; do
            relative="${entry#"$package_dir"/}"
            if [[ -d "$entry" ]]; then
                printf '%s/%s/\n' "$package_name" "$relative"
            else
                printf '%s/%s\n' "$package_name" "$relative"
            fi
        done < <(find "$package_dir" -mindepth 1 -print0 | LC_ALL=C sort -z)
    } | LC_ALL=C sort >"$expected_members"
    tar -tzf "$archive_path" | LC_ALL=C sort >"$actual_members"
    if [[ -n "$(uniq -d "$actual_members")" ]]; then
        printf 'release inventory: tarball contains duplicate member names\n' >&2
        exit 1
    fi
    cmp -s "$expected_members" "$actual_members" || {
        printf 'release inventory: tarball member names differ from the physical package\n' >&2
        exit 1
    }
    while IFS= read -r listing; do
        case "${listing:0:1}" in
            -|d)
                ;;
            *)
                printf 'release inventory: tarball contains a link or special member\n' >&2
                exit 1
                ;;
        esac
    done < <(tar -tvzf "$archive_path")

    member_count=0
    total_size=0
    while IFS= read -r -d '' entry; do
        [[ -f "$entry" && ! -L "$entry" ]] || continue
        relative="${entry#"$package_dir"/}"
        member_size="$(stat -c '%s' "$entry")"
        ((member_count += 1))
        ((total_size += member_size))
        if ((member_count > 4096 || total_size > 1073741824)); then
            printf 'release inventory: package exceeds bounded verification limits\n' >&2
            exit 1
        fi
        tar -xOzf "$archive_path" -- "$package_name/$relative" \
            | cmp -s - "$entry" || {
                printf 'release inventory: archive content differs: %s\n' \
                    "$relative" >&2
                exit 1
            }
    done < <(find "$package_dir" -type f -print0 | LC_ALL=C sort -z)

    manifest="$package_dir/release-manifest.json"
    if [[ -f "$manifest" && ! -L "$manifest" ]]; then
        binary_expected="$(jq -er '.binary_sha256' "$manifest")"
        binary_actual="$(sha256sum "$expected_binary" | awk '{print $1}')"
        [[ "$binary_actual" == "$binary_expected" ]] || {
            printf 'release inventory: physical binary digest differs from manifest\n' >&2
            exit 1
        }
        while IFS= read -r encoded; do
            relative="$(jq -er '.[0]' <<<"$encoded")"
            expected_sha="$(jq -er '.[1]' <<<"$encoded")"
            artifact="$package_dir/$relative"
            [[ -f "$artifact" && ! -L "$artifact" \
                && "$(sha256sum "$artifact" | awk '{print $1}')" \
                    == "$expected_sha" ]] || {
                printf 'release inventory: physical artifact digest differs: %s\n' \
                    "$relative" >&2
                exit 1
            }
        done < <(
            jq -c '.artifact_hashes | to_entries[]
                | [.key, .value]' "$manifest"
        )
    fi

    if [[ -n "${REDLINE_TESTING_TEST_ARCHIVE_OPEN_SIGNAL:-}" ]]; then
        [[ "${REDLINE_TESTING_HOSTILE_TEST:-0}" == 1 ]] || {
            printf 'release inventory: test synchronization is forbidden\n' >&2
            exit 1
        }
        signal_path="$REDLINE_TESTING_TEST_ARCHIVE_OPEN_SIGNAL"
        continue_path="${signal_path}.continue"
        [[ "$signal_path" == /tmp/* && ! -e "$signal_path" \
            && ! -L "$signal_path" && ! -e "$continue_path" \
            && ! -L "$continue_path" ]] || {
            printf 'release inventory: unsafe hostile synchronization path\n' >&2
            exit 1
        }
        : >"$signal_path"
        for _wait in $(seq 1 200); do
            [[ -e "$continue_path" ]] && break
            sleep 0.01
        done
        [[ -f "$continue_path" && ! -L "$continue_path" ]] || {
            printf 'release inventory: hostile synchronization timed out\n' >&2
            exit 1
        }
    fi

    [[ ! -L "$tarball" && -f "$tarball" \
        && "$(stat -Lc '%d:%i:%h:%s:%F' "$tarball")" \
            == "$archive_identity" ]] || {
        printf 'release inventory: archive pathname changed during verification\n' >&2
        exit 1
    }
    archive_sha="$(sha256sum "$archive_path" | awk '{print $1}')"
    [[ "$archive_sha" =~ ^[0-9a-f]{64}$ ]] || {
        printf 'release inventory: archive digest is not canonical\n' >&2
        exit 1
    }
    sidecar_parent="$(dirname "$sidecar")"
    mkdir -p "$sidecar_parent"
    sidecar_tmp="$(mktemp "$sidecar_parent/.release-sidecar.XXXXXX")"
    printf '%s  %s\n' "$archive_sha" "$(basename "$tarball")" >"$sidecar_tmp"
    chmod 0644 "$sidecar_tmp"
    mv -f -- "$sidecar_tmp" "$sidecar"
    [[ "$(stat -Lc '%d:%i:%h:%s:%F' "$tarball")" \
        == "$archive_identity" ]] || {
        rm -f -- "$sidecar"
        printf 'release inventory: archive pathname changed during digest publication\n' >&2
        exit 1
    }
fi

printf 'release inventory: closed bin and package inventory verified\n'
