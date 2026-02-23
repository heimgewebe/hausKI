#!/usr/bin/env python3
"""Stub zum Erzeugen von semantAH-Indexartefakten."""

from __future__ import annotations

import argparse
import json
import os
import urllib.request
from pathlib import Path
from typing import Any

DEFAULT_NAMESPACE = "default"
DEFAULT_OLLAMA_URL = "http://127.0.0.1:11434"
DEFAULT_MODEL = "nomic-embed-text"


class OllamaEmbedder:
    """Kommuniziert mit der Ollama-API zum Erzeugen von Embeddings."""

    def __init__(self, url: str, model: str):
        self.url = url.rstrip("/")
        self.model = model

    def embed(self, texts: list[str]) -> list[list[float]]:
        """Erzeugt Embeddings für eine Liste von Texten."""
        if not texts:
            return []

        # Ollama /api/embed Endpoint
        payload = json.dumps({"model": self.model, "input": texts}).encode("utf-8")
        req = urllib.request.Request(
            f"{self.url}/api/embed",
            data=payload,
            headers={"Content-Type": "application/json"},
        )

        try:
            with urllib.request.urlopen(req) as resp:
                result = json.loads(resp.read().decode("utf-8"))
                return result.get("embeddings", [])
        except Exception as e:
            print(f"[semantah] Warnung: Fehler beim Aufruf von Ollama ({self.url}): {e}")
            # Fallback: Leere Embeddings zurückgeben
            return [[] for _ in texts]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="semantAH Index-Generator")
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
    parser.add_argument(
        "--ollama-url",
        default=os.environ.get("HAUSKI_OLLAMA_URL", DEFAULT_OLLAMA_URL),
        help="URL zur Ollama-API",
    )
    parser.add_argument(
        "--model",
        default=os.environ.get("HAUSKI_EMBED_MODEL", DEFAULT_MODEL),
        help="Name des Embedding-Modells",
    )
    return parser.parse_args()


def ensure_dirs(base: Path) -> Path:
    base.mkdir(parents=True, exist_ok=True)
    gewebe = base / ".gewebe"
    gewebe.mkdir(exist_ok=True)
    return gewebe


def write_embeddings(gewebe: Path, chunks: list[Path], embedder: OllamaEmbedder) -> None:
    """Erzeugt Embeddings und schreibt sie als Parquet und JSON-Manifest."""
    parquet_path = gewebe / "embeddings.parquet"
    manifest_path = gewebe / "chunks.json"

    # 1. Texte lesen und Metadaten vorbereiten
    texts: list[str] = []
    chunk_meta: list[dict[str, Any]] = []

    for chunk_path in chunks:
        try:
            content = chunk_path.read_text(encoding="utf-8")
            texts.append(content)
            chunk_meta.append(
                {
                    "chunk_id": chunk_path.stem,
                    "source": str(chunk_path),
                    "namespace": gewebe.parent.name,
                    "text": content,
                }
            )
        except Exception as e:
            print(f"[semantah] Fehler beim Lesen von {chunk_path}: {e}")

    # 2. Embeddings erzeugen
    embeddings = embedder.embed(texts)

    # 3. Daten zusammenführen
    manifest_data: list[dict[str, Any]] = []
    for meta, emb in zip(chunk_meta, embeddings):
        manifest_data.append({**meta, "embedding": emb})

    # 4. Parquet schreiben (erfordert pyarrow)
    try:
        import pyarrow as pa
        import pyarrow.parquet as pq

        # Liste von Dicts in Arrow-Table konvertieren
        table = pa.Table.from_pylist(manifest_data)
        pq.write_table(table, parquet_path)
        print(f"[semantah] {len(manifest_data)} Embeddings in {parquet_path} geschrieben")
    except ImportError:
        print("[semantah] Warnung: pyarrow nicht gefunden. Parquet-Datei ist ein Platzhalter.")
        parquet_path.write_text(
            "Fehler: pyarrow nicht installiert. Embeddings nur im JSON-Manifest verfügbar.\n"
        )

    # 5. JSON-Manifest schreiben
    manifest_path.write_text(
        json.dumps(manifest_data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def write_report(gewebe: Path, chunks: list[Path]) -> None:
    reports = gewebe / "reports"
    reports.mkdir(exist_ok=True)
    report_path = reports / "index_report.md"
    lines = ["# semantAH Index Report", "", f"Chunks verarbeitet: {len(chunks)}"]
    lines.append(f"Parquet-Artefakt: {gewebe / 'embeddings.parquet'}")
    lines.append(f"Manifest: {gewebe / 'chunks.json'}")
    report_path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> None:
    args = parse_args()
    namespace_dir = Path(args.index_path).expanduser() / args.namespace
    gewebe = ensure_dirs(namespace_dir)

    embedder = OllamaEmbedder(args.ollama_url, args.model)
    chunk_paths = [Path(chunk) for chunk in args.chunks] if args.chunks else []
    write_embeddings(gewebe, chunk_paths, embedder)
    write_report(gewebe, chunk_paths)

    print(f"[semantah] embeddings aktualisiert unter {gewebe}")


if __name__ == "__main__":
    main()
