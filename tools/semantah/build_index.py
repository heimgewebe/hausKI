#!/usr/bin/env python3
"""Stub zum Erzeugen von semantAH-Indexartefakten."""

from __future__ import annotations

import argparse
import contextlib
import json
import os
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

DEFAULT_NAMESPACE = "default"
DEFAULT_OLLAMA_URL = "http://127.0.0.1:11434"
DEFAULT_MODEL = "nomic-embed-text"


class OllamaEmbedder:
    """Kommuniziert mit der Ollama-API zum Erzeugen von Embeddings."""

    def __init__(self, url: str, model: str, allow_empty: bool = False):
        self.url = url.rstrip("/")
        self.model = model
        self.allow_empty = allow_empty

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
            with urllib.request.urlopen(req, timeout=30) as resp:
                result = json.loads(resp.read().decode("utf-8"))
                embeddings = result.get("embeddings")
                if not isinstance(embeddings, list):
                    raise ValueError(f"Ungültiges Antwortformat von Ollama: {result}")
                if len(embeddings) != len(texts):
                    raise ValueError(
                        f"Anzahl Embeddings ({len(embeddings)}) "
                        f"entspricht nicht Input ({len(texts)})"
                    )
                return embeddings
        except (
            urllib.error.HTTPError,
            urllib.error.URLError,
            TimeoutError,
            json.JSONDecodeError,
            ValueError,
        ) as e:
            msg = (
                f"[semantah] Fehler beim Aufruf von Ollama "
                f"({self.url}, Modell: {self.model}): {e}"
            )
            if self.allow_empty:
                print(f"WARNUNG: {msg} (Fahre fort wegen --allow-empty-embeddings)")
                return [[] for _ in texts]

            raise RuntimeError(msg) from e


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
    parser.add_argument(
        "--allow-empty-embeddings",
        action="store_true",
        help="Fahre bei Embedding-Fehlern fort und erzeuge leere Vektoren (nicht empfohlen)",
    )
    return parser.parse_args()


def ensure_dirs(base: Path) -> Path:
    base.mkdir(parents=True, exist_ok=True)
    gewebe = base / ".gewebe"
    gewebe.mkdir(exist_ok=True)
    return gewebe


def write_embeddings(
    gewebe: Path, chunks: list[Path], embedder: OllamaEmbedder
) -> dict[str, int]:
    """Erzeugt Embeddings und schreibt sie als Parquet und JSON-Manifest.
    Gibt Statistik über verarbeitete Chunks zurück.
    """
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
        except (OSError, UnicodeDecodeError) as e:
            print(f"[semantah] Fehler beim Lesen von {chunk_path}: {e}")

    # 2. Embeddings erzeugen
    embeddings = embedder.embed(texts)

    # 3. Daten zusammenführen
    if len(chunk_meta) != len(embeddings):
        raise RuntimeError(
            f"Datenintegritätsfehler: Chunks ({len(chunk_meta)}) != Embeddings ({len(embeddings)})"
        )

    manifest_data: list[dict[str, Any]] = []
    for meta, emb in zip(chunk_meta, embeddings, strict=False):
        manifest_data.append({**meta, "embedding": emb})

    # 4. Parquet schreiben (erfordert pyarrow)
    try:
        import pyarrow as pa
        import pyarrow.parquet as pq

        table = pa.Table.from_pylist(manifest_data)
        pq.write_table(table, parquet_path)
        print(f"[semantah] {len(manifest_data)} Embeddings in {parquet_path} geschrieben")
        # Aufräumen falls Hinweis-Datei existierte
        hint_file = gewebe / "embeddings.parquet.MISSING_PYARROW.txt"
        if hint_file.exists():
            hint_file.unlink()
    except ImportError:
        print("[semantah] Warnung: pyarrow nicht gefunden. Parquet-Datei wird übersprungen.")
        (gewebe / "embeddings.parquet.MISSING_PYARROW.txt").write_text(
            "Parquet-Export übersprungen, da 'pyarrow' nicht installiert ist.\n"
            "Nutze 'pip install pyarrow' zum Aktivieren.\n"
        )
        if parquet_path.exists():
            with contextlib.suppress(Exception):
                parquet_path.unlink()

    # 5. JSON-Manifest schreiben
    manifest_path.write_text(
        json.dumps(manifest_data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )

    return {
        "passed": len(chunks),
        "read": len(chunk_meta),
        "embedded": len(manifest_data),
    }


def write_report(gewebe: Path, stats: dict[str, int]) -> None:
    reports = gewebe / "reports"
    reports.mkdir(exist_ok=True)
    report_path = reports / "index_report.md"
    lines = [
        "# semantAH Index Report",
        "",
        f"- Chunks übergeben: {stats.get('passed', 0)}",
        f"- Chunks gelesen: {stats.get('read', 0)}",
        f"- Embeddings erzeugt: {stats.get('embedded', 0)}",
    ]

    parquet_file = gewebe / "embeddings.parquet"
    if parquet_file.exists():
        lines.append(f"Parquet-Artefakt: {parquet_file}")
    else:
        lines.append("Parquet-Artefakt: (Übersprungen oder Fehler)")

    lines.append(f"Manifest: {gewebe / 'chunks.json'}")
    report_path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> None:
    args = parse_args()
    namespace_dir = Path(args.index_path).expanduser() / args.namespace
    gewebe = ensure_dirs(namespace_dir)

    embedder = OllamaEmbedder(args.ollama_url, args.model, allow_empty=args.allow_empty_embeddings)
    chunk_paths = [Path(chunk) for chunk in args.chunks] if args.chunks else []
    stats = write_embeddings(gewebe, chunk_paths, embedder)
    write_report(gewebe, stats)

    print(f"[semantah] embeddings aktualisiert unter {gewebe}")


if __name__ == "__main__":
    main()
