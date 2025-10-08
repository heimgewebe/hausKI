#!/usr/bin/env python3
"""Related-Writer für semantAH.

Der Writer ersetzt den Block `<!-- related:auto:start --> … <!-- related:auto:end -->`
idempotent. Er nutzt die stubhaften Graph-Artefakte und erzeugt deterministische
Ausgaben. Mit `--check` wird nur ein Diff erzeugt.
"""

from __future__ import annotations

import argparse
import difflib
import json
import os
import re
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path

DEFAULT_NAMESPACE = "default"
MARKER_START = "<!-- related:auto:start -->"
MARKER_END = "<!-- related:auto:end -->"
BLOCK_PATTERN = re.compile(
    r"<!-- related:auto:start -->.*?<!-- related:auto:end -->", re.DOTALL
)


@dataclass
class Edge:
    source: str
    target: str
    score: float


@dataclass
class Node:
    node_id: str
    path: str
    title: str


@dataclass
class RelatedBlock:
    auto: list[tuple[str, float]]
    review: list[tuple[str, float]]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="semantAH Related Writer")
    parser.add_argument(
        "--index-path",
        default=os.environ.get(
            "HAUSKI_INDEX_PATH", os.path.expandvars("$HOME/.local/state/hauski/index")
        ),
        help="Basisverzeichnis für den Index",
    )
    parser.add_argument("--namespace", default=DEFAULT_NAMESPACE, help="Namespace")
    parser.add_argument(
        "--vault",
        default=os.environ.get("HAUSKI_OBSIDIAN_VAULT", "seeds/obsidian.sample"),
        help="Pfad zum Obsidian-Vault",
    )
    parser.add_argument(
        "--note", help="Nur eine spezifische Note aktualisieren (relativer Pfad)"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Nur Diff anzeigen, keine Dateien schreiben",
    )
    return parser.parse_args()


def gewebe_dir(index_path: Path, namespace: str) -> Path:
    return index_path.expanduser() / namespace / ".gewebe"


def load_nodes(path: Path) -> dict[str, Node]:
    mapping: dict[str, Node] = {}
    nodes_file = path / "nodes.jsonl"
    if not nodes_file.exists():
        return mapping

    with nodes_file.open("r", encoding="utf-8") as handle:
        for line in handle:
            data = json.loads(line)
            node = Node(
                node_id=data.get("id", ""),
                path=data.get("path", ""),
                title=data.get("title", data.get("id", "")),
            )
            mapping[node.node_id] = node
    return mapping


def load_edges(path: Path) -> list[Edge]:
    edges_path = path / "edges.jsonl"
    edges: list[Edge] = []
    if not edges_path.exists():
        return edges

    with edges_path.open("r", encoding="utf-8") as handle:
        for line in handle:
            data = json.loads(line)
            edges.append(
                Edge(
                    source=data.get("source", ""),
                    target=data.get("target", ""),
                    score=float(data.get("score", 0.0)),
                )
            )
    return edges


def build_related_map(
    nodes: dict[str, Node], edges: Iterable[Edge]
) -> dict[str, RelatedBlock]:
    related: dict[str, RelatedBlock] = {}

    for edge in edges:
        source_node = nodes.get(edge.source)
        target_node = nodes.get(edge.target)
        if source_node is None or target_node is None:
            continue

        label_list = None
        if edge.score >= 0.80:
            label_list = "auto"
        elif 0.70 <= edge.score < 0.80:
            label_list = "review"

        if label_list is None:
            continue

        block = related.setdefault(source_node.path, RelatedBlock(auto=[], review=[]))
        getattr(block, label_list).append((target_node.title, edge.score))

    for block in related.values():
        block.auto.sort(key=lambda item: (-item[1], item[0]))
        block.review.sort(key=lambda item: (-item[1], item[0]))

    return related


def render_block(block: RelatedBlock) -> str:
    lines = [MARKER_START]
    if block.auto:
        for title, score in block.auto:
            lines.append(f"- auto · {score:.2f} · [[{title}]]")
    if block.review:
        for title, score in block.review:
            lines.append(f"- review · {score:.2f} · [[{title}]]")
    if not block.auto and not block.review:
        lines.append("- (keine Treffer)")
    lines.append(MARKER_END)
    return "\n".join(lines) + "\n"


def upsert_block(content: str, rendered: str) -> tuple[str, bool]:
    if BLOCK_PATTERN.search(content):
        new_content = BLOCK_PATTERN.sub(rendered.strip("\n"), content)
        # Stelle sicher, dass der Block auf einer eigenen Zeile endet
        if not new_content.endswith("\n"):
            new_content += "\n"
        return new_content, new_content != content

    if content and not content.endswith("\n"):
        content += "\n"
    new_content = content + rendered
    return new_content, True


def update_notes(
    vault: Path,
    related_map: dict[str, RelatedBlock],
    check_only: bool,
    single_note: str | None,
) -> None:
    for relative_path, block in sorted(related_map.items()):
        if single_note and relative_path != single_note:
            continue
        note_path = vault / relative_path
        note_path.parent.mkdir(parents=True, exist_ok=True)
        original = note_path.read_text(encoding="utf-8") if note_path.exists() else ""
        rendered = render_block(block)
        updated, changed = upsert_block(original, rendered)
        if check_only:
            if changed:
                diff = difflib.unified_diff(
                    original.splitlines(keepends=True),
                    updated.splitlines(keepends=True),
                    fromfile=str(note_path),
                    tofile=f"{note_path} (preview)",
                )
                print("".join(diff))
        else:
            if changed:
                note_path.write_text(updated, encoding="utf-8")
                print(f"[semantah] related aktualisiert: {note_path}")


def main() -> None:
    args = parse_args()
    index_path = Path(args.index_path)
    vault = Path(args.vault).expanduser()
    gewebe = gewebe_dir(index_path, args.namespace)
    nodes = load_nodes(gewebe)
    edges = load_edges(gewebe)
    related_map = build_related_map(nodes, edges)

    if not related_map:
        # Nichts zu tun, aber Marker optional erzeugen
        if args.note:
            related_map[args.note] = RelatedBlock(auto=[], review=[])
        else:
            print("[semantah] keine Kanten gefunden - keine Notizen aktualisiert")
            return

    update_notes(vault, related_map, args.check, args.note)


if __name__ == "__main__":
    main()
