#!/usr/bin/env bash
# Compare two jankurai score documents and reject score/finding/cap regressions.

set -euo pipefail

if [ "$#" -ne 3 ]; then
    printf 'usage: %s <before.json> <after.json> <commit|push>\n' "$0" >&2
    exit 64
fi

before="$1"
after="$2"
operation="$3"

checks="$(jq -cn --slurpfile before "$before" --slurpfile after "$after" '
def integer_metric($doc; $name):
  ($doc[$name] // $doc.decision[$name]) as $value
  | if (($value | type) == "number" and ($value | floor) == $value)
    then $value else null end;
def score_metric($doc; $name):
  $doc[$name] as $value
  | if ($value | type) == "number" then $value else null end;
def caps($doc):
  ($doc.caps_applied // [])
  | if type == "array" then map(tostring) | unique else [] end;
def finding_count($doc):
  integer_metric($doc; "finding_count") as $count
  | if $count != null then $count
    else (integer_metric($doc; "hard_findings")) as $hard
    | (integer_metric($doc; "soft_findings")) as $soft
    | if ($hard != null and $soft != null) then $hard + $soft else null end
    end;

$before[0] as $old
| $after[0] as $new
| (caps($old)) as $old_caps
| (caps($new)) as $new_caps
| ($new_caps - $old_caps) as $added_caps
| (
    [
      ["score", "raw_score"][] as $name
      | (score_metric($old; $name)) as $before_value
      | (score_metric($new; $name)) as $after_value
      | select($before_value != null and $after_value != null and $after_value < $before_value)
      | "\($name) decreased: \($before_value) -> \($after_value)"
    ]
    + [
      ["hard_findings", "soft_findings"][] as $name
      | (integer_metric($old; $name)) as $before_value
      | (integer_metric($new; $name)) as $after_value
      | select($before_value != null and $after_value != null and $after_value > $before_value)
      | "\($name) increased: \($before_value) -> \($after_value)"
    ]
    + [
      (finding_count($old)) as $before_value
      | (finding_count($new)) as $after_value
      | select($before_value != null and $after_value != null and $after_value > $before_value)
      | "finding_count increased: \($before_value) -> \($after_value)"
    ]
    + [
      select(($new_caps | length) > ($old_caps | length))
      | "applied cap count increased: \($old_caps | length) -> \($new_caps | length)"
    ]
    + [
      select(($added_caps | length) > 0)
      | "new applied caps: \($added_caps | join(", "))"
    ]
  )
')"

if [ "$(printf '%s\n' "$checks" | jq 'length')" -ne 0 ]; then
    printf 'ERROR: score ratchet rejected this %s:\n' "$operation" >&2
    printf '%s\n' "$checks" | jq -r '.[] | "  - \(.)"' >&2
    exit 1
fi

printf 'Score ratchet passed.\n'
