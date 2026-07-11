set shell := ["bash", "-euo", "pipefail", "-c"]

default: validate

validate:
  ./scripts/guard-no-duplicate-engine.sh

doctor:
  REDLINE_SPLIT_ROOT="{{invocation_directory()}}/.." ../redline-split-ops/redlinectl doctor

family-validate:
  REDLINE_SPLIT_ROOT="{{invocation_directory()}}/.." ../redline-split-ops/redlinectl validate
