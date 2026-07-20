from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/verify_heimlern_freeze.py"


def _load_validator():
    spec = importlib.util.spec_from_file_location("verify_heimlern_freeze", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_current_heimlern_freeze_is_valid() -> None:
    module = _load_validator()
    result = module.validate()
    assert result["status"] == "valid"
    assert result["remote_heimlern_dependencies"] == 0
    assert result["deprecated_direct_paths"] == 0


def test_vendor_digest_mismatch_fails_closed(tmp_path: Path, monkeypatch) -> None:
    module = _load_validator()
    manifest = json.loads(module.MANIFEST_PATH.read_text(encoding="utf-8"))
    manifest["vendored_packages"][0]["tree_sha256"] = "0" * 64
    altered = tmp_path / "freeze.json"
    altered.write_text(json.dumps(manifest), encoding="utf-8")
    monkeypatch.setattr(module, "MANIFEST_PATH", altered)

    with pytest.raises(module.FreezeError, match="tree digest changed"):
        module.validate()


def test_remote_runtime_marker_fails_closed(tmp_path: Path, monkeypatch) -> None:
    module = _load_validator()
    marker_file = tmp_path / "runtime.toml"
    marker_file.write_text(
        "source = 'https://github.com/heimgewebe/heimlern'\n",
        encoding="utf-8",
    )
    monkeypatch.setattr(module, "SCAN_ROOTS", (tmp_path,))

    with pytest.raises(module.FreezeError, match="forbidden Heimlern runtime marker"):
        module.validate()
