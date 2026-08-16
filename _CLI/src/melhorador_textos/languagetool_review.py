# ==============================================================================
# SCRIPT: languagetool_review.py
# DESCRIÇÃO: Prepara revisão manual no LanguageTool Premium e importa o corrigido
# CHAMADO POR: melhorador_textos.cli (comandos prepare-lt, import-lt)
# DEPENDÊNCIAS: hashlib, json, difflib, textwrap
# CONTRATO (RESPOSTA ESPERADA): caminhos gerados + diff unificado (str)
# ==============================================================================
"""Fluxo manual auditável para o LanguageTool Premium.

Como a assinatura atual não fornece credenciais da Proofreading API, não
chamamos a API. Em vez disso:
- prepare_package: gera um TXT para colar no editor Premium + manifesto com
  hash do texto original, para garantir rastreabilidade.
- import_correction: recebe a versão corrigida colada de volta, valida contra
  o manifesto e gera um diff unificado revisável. Nada é aplicado sozinho.
"""

from __future__ import annotations

import difflib
import hashlib
import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

# Limite de caracteres por request do LanguageTool Premium.
# Usamos margem de segurança abaixo dos 60.000 documentados.
_LT_PREMIUM_CHAR_LIMIT = 60_000
_LT_SAFE_CHUNK = 50_000


def sha256_text(text: str) -> str:
    """Hash SHA-256 do texto em UTF-8, para trilha de auditoria."""
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def chunk_text(text: str, *, size: int = _LT_SAFE_CHUNK) -> list[str]:
    """Divide o texto em blocos por parágrafo, respeitando o limite da API.

    Nunca corta no meio de um parágrafo se puder evitar; só divide um
    parágrafo gigante quando ele sozinho excede o limite.
    """
    paragraphs = text.split("\n\n")
    chunks: list[str] = []
    current = ""
    for para in paragraphs:
        candidate = para if not current else f"{current}\n\n{para}"
        if len(candidate) <= size:
            current = candidate
            continue
        if current:
            chunks.append(current)
            current = ""
        if len(para) <= size:
            current = para
        else:
            # Parágrafo maior que o limite: fatiar em pedaços de `size`.
            for i in range(0, len(para), size):
                chunks.append(para[i : i + size])
    if current:
        chunks.append(current)
    return chunks


@dataclass
class ReviewPackage:
    """Artefatos gerados para revisão manual."""

    original_path: Path
    manifest_path: Path
    chunks: int


def prepare_package(cleaned_text: str, out_dir: Path) -> ReviewPackage:
    """Gera o pacote de revisão manual do LanguageTool.

    Escreve original.txt (texto a colar) e manifest.json (hash + instruções +
    metadados de chunking). Não contém credenciais.
    """
    out_dir.mkdir(parents=True, exist_ok=True)
    original_path = out_dir / "original.txt"
    manifest_path = out_dir / "manifest.json"

    original_path.write_text(cleaned_text, encoding="utf-8")
    chunks = chunk_text(cleaned_text)

    manifest = {
        "created_at": datetime.now(timezone.utc).isoformat(),
        "language": "pt-BR",
        "source_sha256": sha256_text(cleaned_text),
        "char_count": len(cleaned_text),
        "chunk_count": len(chunks),
        "char_limit_per_request": _LT_PREMIUM_CHAR_LIMIT,
        "chunk_sizes": [len(c) for c in chunks],
        "instructions": [
            "Abrir o editor Premium do LanguageTool em pt-BR.",
            "Colar o conteúdo de original.txt (respeitar a divisão em chunks se necessário).",
            "Revisar manualmente cada sugestão; não aceitar mudanças que alterem sentido, nomes próprios ou grafia histórica.",
            "Salvar o texto revisado como corrected.md na mesma pasta.",
            "Rodar: melhorador-textos import-lt --original original.txt --corrected corrected.md",
        ],
    }
    manifest_path.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    return ReviewPackage(
        original_path=original_path,
        manifest_path=manifest_path,
        chunks=len(chunks),
    )


def build_diff(original_text: str, corrected_text: str) -> str:
    """Gera diff unificado entre original e corrigido (revisável)."""
    diff = difflib.unified_diff(
        original_text.splitlines(keepends=True),
        corrected_text.splitlines(keepends=True),
        fromfile="original",
        tofile="corrected",
    )
    return "".join(diff)


@dataclass
class ImportResult:
    """Resultado da reimportação da versão corrigida."""

    diff: str
    diff_path: Path
    changed: bool
    source_hash_matches: bool | None


def import_correction(
    original_path: Path,
    corrected_path: Path,
    out_dir: Path,
    *,
    manifest_path: Path | None = None,
) -> ImportResult:
    """Compara corrigido vs original e grava changes.diff.

    Se um manifesto for fornecido, valida que o original não mudou desde a
    preparação (proteção contra revisar em cima de texto errado).
    """
    original_text = original_path.read_text(encoding="utf-8")
    corrected_text = corrected_path.read_text(encoding="utf-8")

    source_hash_matches: bool | None = None
    if manifest_path and manifest_path.exists():
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        expected = manifest.get("source_sha256")
        if expected:
            source_hash_matches = expected == sha256_text(original_text)

    diff = build_diff(original_text, corrected_text)
    out_dir.mkdir(parents=True, exist_ok=True)
    diff_path = out_dir / "changes.diff"
    diff_path.write_text(diff, encoding="utf-8")

    return ImportResult(
        diff=diff,
        diff_path=diff_path,
        changed=bool(diff),
        source_hash_matches=source_hash_matches,
    )
