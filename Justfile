set shell := ["bash", "-euo", "pipefail", "-c"]

redlinectl := "{{invocation_directory()}}/redlinectl"

doctor:
  {{redlinectl}} doctor

validate:
  {{redlinectl}} validate

lock-verify:
  {{redlinectl}} lock-verify

clone-dry-run:
  {{redlinectl}} clone --dry-run

update:
  {{redlinectl}} update

family-ci:
  {{redlinectl}} family-ci
