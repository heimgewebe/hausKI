#!/usr/bin/env python3
"""Fail closed when HausKI regains a runtime dependency on archived Heimlern."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = ROOT / "docs/contracts/heimlern-compatibility-freeze.v1.json"
SELF_PATH = Path(__file__).resolve()
SCAN_ROOTS = (
    ROOT / "Cargo.toml",
    ROOT / "Cargo.lock",
    ROOT / ".cargo",
    ROOT / ".github",
    ROOT / ".systemd",
    ROOT / "crates",
    ROOT / "scripts",
)


class FreezeError(RuntimeError):
    """Raised when the compatibility freeze no longer holds."""


def _tree_sha256(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    files = [candidate for candidate in path.rglob("*") if candidate.is_file()]
    for candidate in sorted(files):
        relative = candidate.relative_to(path).as_posix()
        content_sha = hashlib.sha256(candidate.read_bytes()).digest()
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0")
        digest.update(content_sha)
    return digest.hexdigest(), len(files)


def _relative(path: str | Path) -> str:
    resolved = Path(path).resolve()
    try:
        return resolved.relative_to(ROOT).as_posix()
    except ValueError:
        return resolved.as_posix()


def _cargo_metadata() -> dict[str, Any]:
    completed = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--no-deps"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise FreezeError(f"cargo metadata failed: {completed.stderr.strip()}")
    value = json.loads(completed.stdout)
    if not isinstance(value, dict):
        raise FreezeError("cargo metadata did not return an object")
    return value


def _package(packages: list[dict[str, Any]], name: str) -> dict[str, Any]:
    matches = [package for package in packages if package.get("name") == name]
    if len(matches) != 1:
        raise FreezeError(f"expected exactly one Cargo package named {name!r}")
    return matches[0]


def _dependency(package: dict[str, Any], name: str) -> dict[str, Any]:
    matches = [dep for dep in package.get("dependencies", []) if dep.get("name") == name]
    if len(matches) != 1:
        raise FreezeError(
            f"expected exactly one dependency {name!r} in package {package.get('name')!r}"
        )
    return matches[0]


def _assert_local_dependency(
    package: dict[str, Any], dependency_name: str, expected_path: str
) -> None:
    dependency = _dependency(package, dependency_name)
    actual_path = dependency.get("path")
    if dependency.get("source") is not None or not isinstance(actual_path, str):
        raise FreezeError(f"{dependency_name} is not a source-free local path dependency")
    if _relative(actual_path) != expected_path:
        raise FreezeError(
            f"{dependency_name} resolves to {_relative(actual_path)!r}, expected {expected_path!r}"
        )


def _scan_files() -> list[Path]:
    files: list[Path] = []
    for root in SCAN_ROOTS:
        if root.is_file():
            files.append(root)
        elif root.is_dir():
            files.extend(candidate for candidate in root.rglob("*") if candidate.is_file())
    return sorted(set(files))


def validate() -> dict[str, Any]:
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    if manifest.get("schema_version") != 1:
        raise FreezeError("unsupported compatibility-freeze schema version")
    if manifest.get("status") != "frozen_local_compatibility":
        raise FreezeError("compatibility freeze is not marked frozen_local_compatibility")

    metadata = _cargo_metadata()
    packages = metadata.get("packages")
    if not isinstance(packages, list):
        raise FreezeError("cargo metadata packages are missing")

    policy_api = _package(packages, "hauski-policy-api")
    core = _package(packages, "heimlern-core")
    bandits = _package(packages, "heimlern-bandits")
    expected_manifests = {
        "hauski-policy-api": "crates/policy_api/Cargo.toml",
        "heimlern-core": "vendor/heimlern-core/Cargo.toml",
        "heimlern-bandits": "vendor/heimlern-bandits/Cargo.toml",
    }
    for package in (policy_api, core, bandits):
        name = str(package["name"])
        if _relative(str(package["manifest_path"])) != expected_manifests[name]:
            raise FreezeError(f"Cargo package {name!r} moved outside its frozen path")
        if package.get("source") is not None:
            raise FreezeError(f"Cargo package {name!r} unexpectedly has a remote source")

    _assert_local_dependency(policy_api, "heimlern-core", "vendor/heimlern-core")
    _assert_local_dependency(policy_api, "heimlern-bandits", "vendor/heimlern-bandits")
    _assert_local_dependency(bandits, "heimlern-core", "vendor/heimlern-core")

    feature = policy_api.get("features", {}).get("heimlern")
    if sorted(feature or []) != ["dep:heimlern-bandits", "dep:heimlern-core"]:
        raise FreezeError("hauski-policy-api heimlern feature no longer binds both local crates")
    if "heimlern" in policy_api.get("features", {}).get("default", []):
        raise FreezeError("heimlern compatibility feature must remain disabled by default")

    package_by_name = {package["name"]: package for package in packages}
    for entry in manifest.get("vendored_packages", []):
        name = entry.get("name")
        if name not in package_by_name:
            raise FreezeError(f"manifest package {name!r} is absent from Cargo metadata")
        path = ROOT / Path(str(entry["manifest_path"])).parent
        tree_sha, file_count = _tree_sha256(path)
        if tree_sha != entry.get("tree_sha256"):
            raise FreezeError(f"vendored package {name!r} tree digest changed")
        if file_count != entry.get("file_count"):
            raise FreezeError(f"vendored package {name!r} file count changed")

    for deprecated in manifest.get("deprecated_paths", []):
        if (ROOT / deprecated).exists():
            raise FreezeError(f"deprecated direct path still exists: {deprecated}")

    excluded = {SELF_PATH, MANIFEST_PATH.resolve()}
    markers = tuple(str(marker) for marker in manifest.get("forbidden_runtime_markers", []))
    for path in _scan_files():
        if path.resolve() in excluded:
            continue
        try:
            content = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        found = [marker for marker in markers if marker in content]
        if found:
            raise FreezeError(f"forbidden Heimlern runtime marker in {_relative(path)}: {found}")

    return {
        "status": "valid",
        "kind": manifest["kind"],
        "cargo_packages": ["hauski-policy-api", "heimlern-core", "heimlern-bandits"],
        "remote_heimlern_dependencies": 0,
        "deprecated_direct_paths": 0,
        "forbidden_runtime_markers": 0,
    }


def main() -> int:
    try:
        result = validate()
    except (FreezeError, KeyError, OSError, ValueError, json.JSONDecodeError) as exc:
        print(json.dumps({"status": "invalid", "error": str(exc)}, sort_keys=True))
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
