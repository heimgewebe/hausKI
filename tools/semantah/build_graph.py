#!/usr/bin/env python3
"""Stub zum Erzeugen eines semantAH-Graphen."""

from __future__ import annotations

import argparse
import json
import os
from collections.abc import Iterable
from pathlib import Path
from typing import Any

DEFAULT_NAMESPACE = "default"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="semantAH Graph-Stub")
    parser.add_argument(
        "--index-path",
        default=os.environ.get(
            "HAUSKI_INDEX_PATH", os.path.expandvars("$HOME/.local/state/hauski/index")
        ),
        help="Basisverzeichnis für den Index",
    )
    parser.add_argument("--namespace", default=DEFAULT_NAMESPACE, help="Namespace")
    return parser.parse_args()


def ensure_dirs(base: Path) -> Path:
    base.mkdir(parents=True, exist_ok=True)
    gewebe = base / ".gewebe"
    gewebe.mkdir(exist_ok=True)
    return gewebe


def build_stub_nodes(namespace: str) -> Iterable[dict[str, Any]]:
    vault_rel = "notes"
    return [
        {
            "id": f"{namespace}/welcome",
            "title": "Welcome to semantAH",
            "path": f"{vault_rel}/welcome.md",
        },
        {
            "id": f"{namespace}/ideas",
            "title": "Ideen",
            "path": f"{vault_rel}/ideas.md",
        },
    ]


def build_stub_edges(nodes: list[dict[str, Any]]) -> Iterable[dict[str, Any]]:
    if len(nodes) < 2:
        return []
    source, target = nodes[0], nodes[1]
    return [
        {
            "source": source["id"],
            "target": target["id"],
            "score": 0.82,
        },
        {
            "source": target["id"],
            "target": source["id"],
            "score": 0.75,
        },
    ]


def write_jsonl(path: Path, items: Iterable[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8") as handle:
        for item in items:
            handle.write(json.dumps(item, ensure_ascii=False) + "\n")


def write_graph(gewebe: Path, namespace: str) -> None:
    nodes = list(build_stub_nodes(namespace))
    edges = list(build_stub_edges(nodes))

    write_jsonl(gewebe / "nodes.jsonl", nodes)
    write_jsonl(gewebe / "edges.jsonl", edges)

    report = gewebe / "reports"
    report.mkdir(exist_ok=True)
    (report / "graph_report.md").write_text(
        "# semantAH Graph Report (Stub)\n", encoding="utf-8"
    )


def main() -> None:
    args = parse_args()
    namespace_dir = Path(args.index_path).expanduser() / args.namespace
    gewebe = ensure_dirs(namespace_dir)
    write_graph(gewebe, args.namespace)
    print(f"[semantah] graph aktualisiert unter {gewebe}")


if __name__ == "__main__":
    main()
