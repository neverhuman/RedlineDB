#!/usr/bin/env bash

set -euo pipefail

package_dir="${1:-}"
tarball="${2:-}"

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
    case "$relative" in
        *$'\n'*|*$'\r'*|*$'\t'*)
            printf 'release inventory: unsafe member name: %q\n' "$relative" >&2
            exit 1
            ;;
    esac
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
    [[ -f "$tarball" && ! -L "$tarball" ]] || {
        printf 'release inventory: tarball is not a physical regular file\n' >&2
        exit 1
    }
    package_name="$(basename "$package_dir")"
    expected_members="$(mktemp)"
    actual_members="$(mktemp)"
    trap 'rm -f "$expected_members" "$actual_members"' EXIT
    {
        printf '%s/\n' "$package_name"
        while IFS= read -r -d '' entry; do
            relative="${entry#"$package_dir"/}"
            if [[ -d "$entry" ]]; then
                printf '%s/%s/\n' "$package_name" "$relative"
            else
                printf '%s/%s\n' "$package_name" "$relative"
            fi
        done < <(find "$package_dir" -mindepth 1 -print0)
    } | LC_ALL=C sort >"$expected_members"
    tar -tzf "$tarball" | LC_ALL=C sort >"$actual_members"
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
    done < <(tar -tvzf "$tarball")
fi

printf 'release inventory: closed bin and package inventory verified\n'
