#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=ops/ci/lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
cd "$repo_root"

for tool in awk realpath sha256sum; do
  require_tool "$tool"
done
require_jankurai

mkdir -p target
fake_dir="$(mktemp -d "$repo_root/target/jankurai-adversarial-path.XXXXXX")"
cleanup() {
  rm -rf -- "$fake_dir"
}
trap cleanup EXIT
printf '#!/usr/bin/env bash\nprintf "jankurai 1.6.11\\n"\n' > "$fake_dir/jankurai"
chmod 0755 "$fake_dir/jankurai"
PATH="$fake_dir:/usr/bin:/bin" require_jankurai
[[ "$(PATH="$fake_dir:/usr/bin:/bin" /bin/bash ops/ci/jankurai.sh --version)" == \
  "jankurai $JANKURAI_VERSION" ]]

if rg -n '\.cargo/bin/jankurai|JANKURAI_VERSION="1\.6\.10"|required_tool_version = "1\.6\.10"' \
  Justfile ops/ci/lib.sh ops/ci/score.sh ops/ci/install-tools.sh \
  agent/audit-policy.toml .github/workflows/jankurai.yml; then
  printf 'ambient or obsolete Jankurai selection remains in an active proof surface\n' >&2
  exit 1
fi
printf 'jankurai doctor ok: exact path/version/digest; adversarial PATH ignored\n'
