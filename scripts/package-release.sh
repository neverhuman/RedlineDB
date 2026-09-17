#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
: "${TAG:?TAG is required}"
case "$(uname -s)/$(uname -m)" in
  Linux/x86_64) platform=linux-x86_64 ;;
  Linux/aarch64|Linux/arm64) platform=linux-arm64 ;;
  Darwin/x86_64) platform=macos-x86_64 ;;
  Darwin/arm64) platform=macos-arm64 ;;
  *) printf 'unsupported packaging platform\n' >&2; exit 1 ;;
esac
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$root/target}
output=${OUTPUT_DIR:-$root/target/packages}
mkdir -p "$output"
output=$(cd "$output" && pwd)
./scripts/build-from-source.sh --all
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
for package in redlinedb redline-web redline-testing; do
  mkdir -p "$stage/$package/bin" "$stage/$package/share/redlinedb/licenses"
  cp LICENSE "$stage/$package/share/redlinedb/LICENSE"
  printf '%s\n' "$TAG" > "$stage/$package/share/redlinedb/VERSION"
done
PREFIX="$stage/redlinedb" ./scripts/install-from-source.sh
install -m 644 contracts/c-abi/sqlite3.h "$stage/redlinedb/include/"
install -m 755 "$CARGO_TARGET_DIR/release/redline-web" "$stage/redline-web/bin/"
install -m 755 "$CARGO_TARGET_DIR/release/redline-testing" "$CARGO_TARGET_DIR/release/redlinedb-client-smoke" "$stage/redline-testing/bin/"
cp -R subrepos/redline-testing/{corpus,metadata,schemas,templates} "$stage/redline-testing/share/redlinedb/"
commit=$(git rev-parse HEAD)
for package in redlinedb redline-web redline-testing; do
  case "$package" in
    redlinedb) manifest=Cargo.toml ;;
    *) manifest=subrepos/$package/Cargo.toml ;;
  esac
  cargo metadata --locked --format-version 1 --manifest-path "$manifest" > "$stage/metadata.json"
  jq '{bomFormat:"CycloneDX",specVersion:"1.5",version:1,components:[.packages[]|{type:"library",name,version,purl:("pkg:cargo/"+.name+"@"+.version),licenses:(if .license then [{expression:.license}] else [] end)}]}' "$stage/metadata.json" > "$stage/$package/share/redlinedb/sbom.cdx.json"
  jq -r '.packages[]|[.name,.version,(.license // "UNKNOWN"),(.repository // "")]|@tsv' "$stage/metadata.json" > "$stage/$package/share/redlinedb/DEPENDENCIES.tsv"
  while IFS=$'\t' read -r name manifest_path; do
    directory=${manifest_path%/Cargo.toml}
    mkdir -p "$stage/$package/share/redlinedb/licenses/$name"
    for license in "$directory"/LICENSE* "$directory"/COPYING* "$directory"/NOTICE*; do
      [[ ! -f $license ]] || cp "$license" "$stage/$package/share/redlinedb/licenses/$name/"
    done
  done < <(jq -r '.packages[]|[ (.name+"-"+.version),.manifest_path]|@tsv' "$stage/metadata.json")
  jq -n --arg commit "$commit" --arg tag "$TAG" --arg platform "$platform" --arg package "$package" --arg rust "$(rustc --version)" '{schema:"redline.release-build/v1",repository:"https://github.com/neverhuman/RedlineDB",commit:$commit,tag:$tag,platform:$platform,package:$package,rust:$rust}' > "$stage/$package/share/redlinedb/build-provenance.json"
  if [[ $package == redline-web ]]; then
    npm --prefix subrepos/redline-web/apps/web sbom --sbom-format cyclonedx > "$stage/$package/share/redlinedb/frontend-sbom.cdx.json"
  fi
  asset=$package-$TAG-$platform.tar.gz
  tar -czf "$output/$asset" -C "$stage/$package" .
  (cd "$output"; if command -v sha256sum >/dev/null; then sha256sum "$asset"; else shasum -a 256 "$asset"; fi) > "$output/$asset.sha256"
done
printf 'Packages written to %s\n' "$output"
