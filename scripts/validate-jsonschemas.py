#!/usr/bin/env python3
"""Validate JSON Schema contracts against Draft 2020-12."""

from __future__ import annotations

import json
import sys
from pathlib import Path

try:
    from jsonschema import Draft202012Validator
except ImportError as exc:  # pragma: no cover - missing optional dependency
    raise SystemExit("jsonschema is required. Install via `uv sync --extra dev`.") from exc


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    schema_paths = sorted(repo_root.glob("docs/contracts/**/*.schema.json"))

    if not schema_paths:
        print("No JSON Schemas found under docs/contracts – skipping.")
        return 0

    exit_code = 0
    for schema_path in schema_paths:
        rel = schema_path.relative_to(repo_root)
        try:
            with schema_path.open("r", encoding="utf-8") as handle:
                schema = json.load(handle)
            Draft202012Validator.check_schema(schema)
        except Exception as exc:
            print(f"✗ {rel}: {exc}")
            exit_code = 1
        else:
            print(f"✓ {rel}")

    return exit_code


if __name__ == "__main__":
    sys.exit(main())
