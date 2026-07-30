#!/usr/bin/env bash

set -euo pipefail

schema="${1:-}"
manifest="${2:-}"
if [[ -z "$schema" || -z "$manifest" ]]; then
    printf 'usage: %s SCHEMA MANIFEST\n' "$0" >&2
    exit 2
fi

jq -e --slurpfile schema "$schema" '
    . as $m
    | $schema[0] as $s
    | select($s."$schema" == "https://json-schema.org/draft/2020-12/schema")
    | select($s.type == "object" and $s.additionalProperties == false)
    | select(($s.required | sort) == ($s.properties | keys | sort))
    | select(type == "object" and (keys | sort) == ($s.required | sort))
    | select(.name == $s.properties.name.const)
    | select((.version | type) == $s.properties.version.type)
    | select((.target | type) == $s.properties.target.type)
    | select(.release_commit | test($s.properties.release_commit.pattern))
    | select(.release_tree | test($s.properties.release_tree.pattern))
    | select(.release_tag | test($s.properties.release_tag.pattern))
    | select($s.properties.release_tag_state.enum | index($m.release_tag_state))
    | select($s.properties.tag_revision.type == "integer"
        and (.tag_revision | type) == "number"
        and (.tag_revision | floor) == .tag_revision
        and .tag_revision >= $s.properties.tag_revision.minimum)
    | select(.source_archive_sha256
        | test($s.properties.source_archive_sha256.pattern))
    | select(.binary == $s.properties.binary.const)
    | select(.binary_sha256 | test($s.properties.binary_sha256.pattern))
    | select(.tarball_sha256_source
        == $s.properties.tarball_sha256_source.const)
    | select((.artifact_hashes | type)
        == $s.properties.artifact_hashes.type)
    | select(all(.artifact_hashes | to_entries[];
        (.key | test($s.properties.artifact_hashes.propertyNames.pattern))
        and (.value
            | test($s.properties.artifact_hashes.additionalProperties.pattern))))
    | select((.build_inputs | type) == $s.properties.build_inputs.type)
    | select((.build_inputs | keys | sort)
        == ($s.properties.build_inputs.required | sort))
    | select($s.properties.build_inputs.additionalProperties == false)
    | select(.build_inputs.source_mode
        == $s.properties.build_inputs.properties.source_mode.const)
    | select(all([
          .build_inputs.cargo_path,
          .build_inputs.rustc_path,
          .build_inputs.cargo_registry_source
        ][]; test($s.properties.build_inputs.properties.cargo_path.pattern)))
    | select(all([
          .build_inputs.cargo_sha256,
          .build_inputs.cargo_version_sha256,
          .build_inputs.rustc_sha256,
          .build_inputs.rustc_version_sha256,
          .build_inputs.cargo_config_sha256,
          .build_inputs.cargo_lock_sha256,
          .build_inputs.cargo_registry_receipt_sha256,
          .build_inputs.cargo_stage_tool_sha256,
          .build_inputs.environment_sha256
        ][]; test($s.properties.build_inputs.properties.cargo_sha256.pattern)))
    | select(.generated_by == $s.properties.generated_by.const)
' "$manifest" >/dev/null || {
    printf 'release manifest: governed schema validation failed: %s\n' \
        "$manifest" >&2
    exit 1
}

printf 'release manifest: governed schema validation passed\n'
