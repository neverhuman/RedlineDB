from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
MATERIALIZE = ROOT / "ops" / "split" / "materialize.py"


def load_materialize():
    spec = importlib.util.spec_from_file_location("materialize", MATERIALIZE)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_manifest_policy_and_source_coverage() -> None:
    module = load_materialize()
    data, repos = module.load_manifest(ROOT / "repos.manifest.toml")
    by_name = {repo.name: repo for repo in repos}
    assert len(repos) == 19
    assert "jain-cli" in by_name
    assert by_name["jain-cli"].cargo_members == ["crates/feat-cli"]
    assert by_name["jain-tui"].cargo_members == ["crates/feat-tui"]
    assert data["source_sha"] == "cc27936eb45006bda0cae85b0f578f4d5985991d"
    assert by_name["jain-starforge"].mirror_github_main is False
    assert "contracts" in by_name["jain-deploy"].copy_paths

    result = subprocess.run(
        ["python3", "ops/split/source_coverage.py", "--json"],
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    )
    coverage = json.loads(result.stdout)
    assert coverage["status"] == "pass"
    assert coverage["missing_count"] == 0
    assert coverage["duplicate_count"] == 0


def test_recursive_owner_and_test_routes_cover_generated_and_source_files() -> None:
    module = load_materialize()
    _, repos = module.load_manifest(ROOT / "repos.manifest.toml")
    core = next(repo for repo in repos if repo.name == "jain-core")
    owners = json.loads(module.render_owner_map(core))["owners"]
    tests = json.loads(module.render_test_map(core))["tests"]

    for key in [".gitattributes", "ops/ci/", "ops/ci/**", "contracts/", "contracts/**", "crates/feat-core/", "crates/feat-core/**"]:
        assert key in owners
        assert key in tests
    assert tests["ops/ci/"]["command"] == "just check && just tool-adoption"


def test_portable_cargo_config_has_no_host_rpaths() -> None:
    module = load_materialize()
    _, repos = module.load_manifest(ROOT / "repos.manifest.toml")
    core = next(repo for repo in repos if repo.name == "jain-core")
    rendered = module.render_cargo_config(core)
    assert "/home/ubuntu/.cache/jain_small" not in rendered
    assert ".cargo" not in module.COMMON_COPY_PATHS
    assert "JAIN_VENDOR_ROOT" in rendered


def test_split_patches_and_starforge_lfs_payloads_exist() -> None:
    module = load_materialize()
    for patch_names in module.SPLIT_PATCHES.values():
        for patch_name in patch_names:
            patch = ROOT / "ops" / "split" / "patches" / patch_name
            assert patch.exists()
            assert patch.stat().st_size > 0

    source_root = Path("/home/ubuntu/jain_small")
    for rel in module.STARFORGE_LFS_PATHS:
        payload = source_root / rel
        assert payload.exists()
        assert payload.stat().st_size > 1_000_000


def test_root_lock_schema_and_stage_context_generation() -> None:
    module = load_materialize()
    _, repos = module.load_manifest(ROOT / "repos.manifest.toml")
    deploy = next(repo for repo in repos if repo.name == "jain-deploy")
    web = next(repo for repo in repos if repo.name == "jain-web")
    lock = module.render_root_lock(repos, "0" * 40)
    assert 'schema_version = "1.0.0"' in lock
    assert 'family = "jain"' in lock
    assert 'repo = "jain-core"' in lock
    assert 'jeryu = "http://127.0.0.1:8787/git/jeryu/jain-core.git"' in lock
    assert "github.com/neverhuman" not in lock
    assert 'excluded_paths = [".stage/"]' in module.render_audit_policy(deploy)
    assert 'path = ".stage/"' in module.generated_zones(deploy)
    web_zones = module.generated_zones(web)
    assert 'path = "apps/web/dist/"' in web_zones
    assert 'write_policy = "auditor_output"' in web_zones
    assert 'format = "generic-json-summary"' in module.render_coverage_sources(web)

    source = MATERIALIZE.read_text()
    assert 'stage / "repos"' in source
    assert '[patch."{remote}"]' in source
    assert "target/jankurai/accepted-baseline.json" in source
    assert "target/jankurai/accepted-baseline.md" in source
    assert "/.jankurai/" in source
    assert "status=pass; [[ $fail -eq 0 ]] || status=fail" in source
    assert '"status":"$status"' in source
    assert (
        '"metrics":{"tabpfn_hits":$tabpfn_hits,'
        '"fallback_advisory_files":$fallback_files}'
    ) in source


def test_local_mirror_config_rewrites_jeryu_and_legacy_github_urls(tmp_path: Path) -> None:
    module = load_materialize()
    _, repos = module.load_manifest(ROOT / "repos.manifest.toml")
    cfg = module.write_git_instead_of_config(tmp_path, repos[:2])
    text = cfg.read_text()
    assert "insteadOf = http://127.0.0.1:8787/git/jeryu/jain.git" in text
    assert "insteadOf = http://127.0.0.1:8787/git/jeryu/jain.git" in text
    assert "target/bare-mirrors/jain.git" in text


def test_generated_operational_sources_use_local_jeryu() -> None:
    module = load_materialize()
    _, repos = module.load_manifest(ROOT / "repos.manifest.toml")
    by_name = {repo.name: repo for repo in repos}
    package_to_repo = {
        "domain": "jain-domain",
        "feat-math": "jain-math",
        "feat-cli": "jain-cli",
    }
    member_to_package = {
        "crates/domain": "domain",
        "crates/feat-math": "feat-math",
        "crates/feat-cli": "feat-cli",
    }
    patch = module.render_patch_sections(by_name["jain-deploy"], repos, package_to_repo, member_to_package)
    clone = module.render_clone_family_script()
    local_patches = module.render_local_patches_example(by_name["jain-core"])
    assert 'git/jeryu/jain-cli.git' in patch
    assert 'remote="http://127.0.0.1:8787/git/${jeryu_slug}.git"' in clone
    assert '[patch."http://127.0.0.1:8787/git/jeryu/jain-core.git"]' in local_patches
    assert "github.com/neverhuman" not in patch
    assert "git clone https://github.com/neverhuman" not in clone
    assert "github.com/neverhuman" not in local_patches
