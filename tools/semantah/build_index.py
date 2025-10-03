#!/usr/bin/env python3
"""Stub zum Erzeugen von semantAH-Indexartefakten."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
from typing import Any

DEFAULT_NAMESPACE = "default"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="semantAH Index-Stub")
    parser.add_argument(
        "--index-path",
        default=os.environ.get(
            "HAUSKI_INDEX_PATH", os.path.expandvars("$HOME/.local/state/hauski/index")
        ),
        help="Basisverzeichnis für den Index",
    )
    parser.add_argument(
        "--namespace",
        default=DEFAULT_NAMESPACE,
        help="Namespace (z. B. default oder obsidian)",
    )
    parser.add_argument(
        "--chunks", nargs="*", help="Optional: Pfade zu Markdown- oder Canvas-Dateien"
    )
    return parser.parse_args()


def ensure_dirs(base: Path) -> Path:
    base.mkdir(parents=True, exist_ok=True)
    gewebe = base / ".gewebe"
    gewebe.mkdir(exist_ok=True)
    return gewebe


def write_embeddings(gewebe: Path, chunks: list[Path]) -> None:
    parquet = gewebe / "embeddings.parquet"
    manifest = gewebe / "chunks.json"
    manifest_data: list[dict[str, Any]] = []

    for chunk_path in chunks:
        manifest_data.append(
            {
                "chunk_id": chunk_path.stem,
                "source": str(chunk_path),
                "namespace": gewebe.parent.name,
                "embedding": "TODO",
            }
        )

    parquet.write_text("placeholder for embeddings parquet\n", encoding="utf-8")
    manifest.write_text(
        json.dumps(manifest_data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def write_report(gewebe: Path, chunks: list[Path]) -> None:
    reports = gewebe / "reports"
    reports.mkdir(exist_ok=True)
    report_path = reports / "index_report.md"
    lines = ["# semantAH Index Report (Stub)", "", f"Chunks verarbeitet: {len(chunks)}"]
    report_path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> None:
    args = parse_args()
    namespace_dir = Path(args.index_path).expanduser() / args.namespace
    gewebe = ensure_dirs(namespace_dir)

    chunk_paths = [Path(chunk) for chunk in args.chunks] if args.chunks else []
    write_embeddings(gewebe, chunk_paths)
    write_report(gewebe, chunk_paths)

    print(f"[semantah] embeddings aktualisiert unter {gewebe}")


if __name__ == "__main__":
    main()
