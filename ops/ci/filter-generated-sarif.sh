#!/usr/bin/env bash
# Remove generated-zone findings from jankurai SARIF before code scanning upload.

set -euo pipefail

usage() {
    printf 'usage: %s <sarif> [--generated-zones <path>]\n' "$0" >&2
}

sarif="${1:-}"
generated_zones=".jankurai/generated-zones.toml"
if [ -z "$sarif" ]; then
    usage
    exit 64
fi
shift
while [ "$#" -gt 0 ]; do
    case "$1" in
        --generated-zones)
            generated_zones="${2:-}"
            if [ -z "$generated_zones" ]; then
                usage
                exit 64
            fi
            shift 2
            ;;
        *)
            usage
            exit 64
            ;;
    esac
done

if [ ! -f "$sarif" ]; then
    printf 'SARIF file not found: %s\n' "$sarif" >&2
    exit 1
fi
if [ ! -f "$generated_zones" ]; then
    printf 'generated zones file not found: %s\n' "$generated_zones" >&2
    exit 1
fi

zones_json="$(
    awk -F'"' '
        /^[[:space:]]*path[[:space:]]*=/ {
            if ($2 != "" && $2 != ".jankurai/generated-zones.toml") {
                print $2
            }
        }
    ' "$generated_zones" \
        | jq -R -s 'split("\n") | map(select(length > 0) | gsub("^\\./"; "") | gsub("\\\\"; "/"))'
)"

count_results() {
    jq '[.runs[]?.results? // [] | length] | add // 0' "$1"
}

before="$(count_results "$sarif")"
tmp="$(mktemp "${sarif}.XXXXXX")"
jq --argjson zones "$zones_json" '
    def normalize_uri:
        gsub("^\\./"; "") | gsub("\\\\"; "/");
    def is_generated($uri):
        ($uri | normalize_uri) as $normalized
        | any($zones[]; . as $zone
            | if ($zone | endswith("/")) then
                ($normalized | startswith($zone))
              else
                $normalized == $zone
              end);
    def result_is_generated:
        any(.locations[]?;
            .physicalLocation.artifactLocation.uri? as $uri
            | is_generated($uri));
    .runs |= (map(.results = ((.results // []) | map(select(result_is_generated | not)))))
' "$sarif" > "$tmp"
mv "$tmp" "$sarif"
after="$(count_results "$sarif")"
removed="$((before - after))"
printf 'filtered generated-zone SARIF findings: before=%s removed=%s after=%s\n' \
    "$before" "$removed" "$after" >&2
