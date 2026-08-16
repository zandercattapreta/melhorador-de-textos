# ==============================================================================
# SCRIPT: check_lt_local.py
# DESCRIÇÃO: Verifica textos limpos com LanguageTool local (open-source)
# CHAMADO POR: batch_check_lt.sh ou CLI futura
# DEPENDÊNCIAS: LanguageTool 6.8+ server, requests
# CONTRATO (RESPOSTA ESPERADA): lt-local-suggestions.json, lt-local-corrected.md
# ==============================================================================
"""Integração com LanguageTool local (open-source, sem sair da máquina).

Processa cleaned.md de um livro contra o servidor LT local (localhost:8081),
grava sugestões em JSON e proposta de correção em Markdown.

Zero-IA: LT é apenas regras gramam/ortográficas, não geração de texto.
Nenhuma sugestão é aplicada sem aprovação humana explícita.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

try:
    import requests
except ImportError:
    requests = None


def check_with_lt_local(
    text: str,
    language: str = "pt",
    lt_url: str = "http://localhost:8081",
) -> dict:
    """Submete texto ao servidor LT local, retorna sugestões brutes.

    Parâmetros:
    - text: texto a verificar (Markdown limpo)
    - language: código de idioma (pt, pt-BR, pt-PT, etc.)
    - lt_url: URL do servidor LT local

    Retorna:
    {
        "matches": [{"message": "...", "offset": X, "length": Y, "replacements": [...], ...}],
        "language": {"name": "...", "code": "..."},
        "warnings": {...}
    }
    """
    if requests is None:
        raise ImportError("requests não instalado. pip install requests")

    try:
        resp = requests.post(
            f"{lt_url}/v2/check",
            data={"text": text, "language": language},
            timeout=30,
        )
        resp.raise_for_status()
        return resp.json()
    except requests.RequestException as e:
        raise RuntimeError(
            f"Falha ao conectar ao LanguageTool local ({lt_url}): {e}. "
            "Inicie o servidor com: languagetool --http --port 8081"
        ) from e


def build_corrected_text(text: str, matches: list[dict]) -> str:
    """Aplica a PRIMEIRA sugestão de cada match (proposta, não final).

    Matches precisam estar ordenados por offset (decrescente) para
    aplicar do fim pro começo, preservando offsets.
    """
    if not matches:
        return text

    # Ordena por offset decrescente para aplicar de trás pra frente
    sorted_matches = sorted(matches, key=lambda m: m["offset"], reverse=True)

    result = text
    for match in sorted_matches:
        offset = match["offset"]
        length = match["length"]
        replacements = match.get("replacements", [])

        if replacements:
            first_replacement = replacements[0]["value"]
            result = result[:offset] + first_replacement + result[offset + length :]

    return result


def check_book(
    cleaned_path: Path,
    output_dir: Path | None = None,
    language: str = "pt",
    lt_url: str = "http://localhost:8081",
) -> dict:
    """Processa um livro completo com LT local.

    Grava:
    - lt-local-suggestions.json (todas as ocorrências)
    - lt-local-corrected.md (proposta com 1ª sugestão de cada)

    Retorna:
    {
        "book": "nome",
        "total_matches": N,
        "language": "pt",
        "suggestions_path": "...",
        "corrected_path": "...",
    }
    """
    if not cleaned_path.exists():
        raise FileNotFoundError(f"Arquivo não encontrado: {cleaned_path}")

    if output_dir is None:
        output_dir = cleaned_path.parent

    output_dir.mkdir(parents=True, exist_ok=True)

    # Lê texto limpo
    text = cleaned_path.read_text(encoding="utf-8")

    # Checa com LT
    result = check_with_lt_local(text, language=language, lt_url=lt_url)
    matches = result.get("matches", [])

    # Grava sugestões brutes
    suggestions_path = output_dir / "lt-local-suggestions.json"
    suggestions_path.write_text(
        json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8"
    )

    # Grava proposta (1ª sugestão aplicada)
    corrected_text = build_corrected_text(text, matches)
    corrected_path = output_dir / "lt-local-corrected.md"
    corrected_path.write_text(corrected_text, encoding="utf-8")

    return {
        "book": cleaned_path.parent.name,
        "total_matches": len(matches),
        "language": language,
        "suggestions_path": str(suggestions_path),
        "corrected_path": str(corrected_path),
    }
