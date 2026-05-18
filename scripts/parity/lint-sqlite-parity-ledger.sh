#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

ledger="docs/sqlite-parity.md"

awk '
BEGIN {
    FS = "|"
}

function trim(value) {
    gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
    return value
}

/^\| Feature row \| Status \|/ {
    in_feature_table = 1
    next
}

in_feature_table && $0 !~ /^\|/ {
    in_feature_table = 0
}

!in_feature_table {
    next
}

/^\|---/ {
    next
}

{
    feature = trim($2)
    status = trim($3)
    notes = trim($6)
    lower_feature = tolower(feature)
    lower_status = tolower(status)
    lower_notes = tolower(notes)

    if (lower_status !~ /^(pass|partial|fail|not-started|rejects-by-design)$/) {
        printf "%s:%d: unknown SQLite parity status %s for %s\n", FILENAME, FNR, status, feature > "/dev/stderr"
        failures = 1
    }

    if (lower_status == "pass" && lower_notes ~ /(incomplete|not complete|remains partial|remain partial|followup|follow-up|still|not yet)/) {
        printf "%s:%d: pass row admits an incomplete/partial gap: %s\n", FILENAME, FNR, feature > "/dev/stderr"
        failures = 1
    }

    if (lower_status == "pass" && lower_feature ~ /pragma/ && (lower_feature ~ /rejected/ || lower_notes ~ /(reject|unsupported)/)) {
        printf "%s:%d: rejected PRAGMA row must not be a parity pass: %s\n", FILENAME, FNR, feature > "/dev/stderr"
        failures = 1
    }

    if (lower_feature ~ /pragma/ && lower_feature ~ /rejected/ && lower_status != "rejects-by-design") {
        printf "%s:%d: rejected PRAGMA row must use rejects-by-design: %s\n", FILENAME, FNR, feature > "/dev/stderr"
        failures = 1
    }
}

END {
    exit failures
}
' "$ledger"
