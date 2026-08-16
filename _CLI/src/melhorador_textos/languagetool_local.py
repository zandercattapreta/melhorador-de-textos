# ==============================================================================
# SCRIPT: languagetool_local.py
# DESCRIÇÃO: Submete texto ao servidor LOCAL do LanguageTool e gera proposta
# CHAMADO POR: cli.py (comando check-lt), batch_extract.py
# DEPENDÊNCIAS: urllib (stdlib), json, subprocess; servidor languagetool-server
# CONTRATO (RESPOSTA ESPERADA): LocalCheckResult; grava lt-local-*.{json,md,diff}
# ==============================================================================
"""Revisão automática via LanguageTool open-source rodando na máquina local.

Camada AUTOMÁTICA do fluxo híbrido de revisão:
- Este módulo chama o servidor local (localhost:8081, regras determinísticas
  da versão open-source — sem IA) e grava três artefatos por texto:
    lt-local-suggestions.json  → todas as ocorrências apontadas pelo LT
    lt-local-corrected.md      → PROPOSTA com a 1ª sugestão de cada ocorrência
    lt-local-changes.diff      → diff cleaned.md → proposta, para aprovação
- NADA é aplicado ao texto canônico: o cleaned.md permanece intacto e o
  humano decide pelo diff (mesmo princípio do fluxo Premium manual).
- A camada MANUAL Premium (prepare-lt/import-lt) continua existindo para a
  passada final — artefatos com nomes distintos (original.txt/corrected.md).

O servidor local não envia nada para a nuvem: o texto do livro não sai da
máquina. Sem o servidor instalado/rodando, o chamador decide se ignora a
etapa (batch) ou falha (comando explícito check-lt).
"""

from __future__ import annotations

import json
import shutil
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

from .languagetool_review import build_diff, chunk_text, sha256_text

# Servidor local padrão (brew services start languagetool).
DEFAULT_SERVER_URL = "http://localhost:8081"

# Chunk para o servidor LOCAL: 20k. O chunk de 50k (dimensionado para o
# editor Premium) derruba o Java com heap padrão (RemoteDisconnected —
# visto em 15/Ago na revisão de livros inteiros).
_LOCAL_SAFE_CHUNK = 20_000
_LANGUAGE = "pt-BR"

# Java demora para subir e carregar o modelo pt na 1ª requisição.
_STARTUP_TIMEOUT_S = 90
_REQUEST_TIMEOUT_S = 120


@dataclass
class LocalCheckResult:
    """Resultado da revisão local com artefatos gravados."""

    matches: list[dict]
    corrected_text: str
    changed: bool
    suggestions_path: Path | None = None
    corrected_path: Path | None = None
    diff_path: Path | None = None
    stats: dict = field(default_factory=dict)


# ==============================================================================
# SERVIDOR
# ==============================================================================

def server_is_up(server_url: str = DEFAULT_SERVER_URL) -> bool:
    """True se o servidor local responde no endpoint /v2/languages."""
    try:
        with urllib.request.urlopen(
            f"{server_url}/v2/languages", timeout=5
        ) as resp:
            return resp.status == 200
    except (urllib.error.URLError, OSError):
        return False


def ensure_server(server_url: str = DEFAULT_SERVER_URL) -> bool:
    """Garante um servidor local de pé; tenta subir um se necessário.

    Sobe `languagetool-server` em background e espera ficar pronto. O processo
    fica vivo após o término do CLI (comportamento de serviço) — para algo
    permanente, o usuário pode preferir `brew services start languagetool`.
    Retorna False se não há binário instalado ou o start falhou.
    """
    if server_is_up(server_url):
        return True

    binary = shutil.which("languagetool-server")
    if binary is None:
        return False

    port = urllib.parse.urlparse(server_url).port or 8081
    # start_new_session desacopla o servidor do ciclo de vida do CLI.
    subprocess.Popen(
        [binary, "--port", str(port), "--allow-origin"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )

    deadline = time.monotonic() + _STARTUP_TIMEOUT_S
    while time.monotonic() < deadline:
        if server_is_up(server_url):
            return True
        time.sleep(1)
    return False


# ==============================================================================
# SUBMISSÃO E APLICAÇÃO DE SUGESTÕES
# ==============================================================================

def _check_chunk(chunk: str, server_url: str) -> list[dict]:
    """Envia um chunk ao /v2/check e devolve a lista de matches."""
    payload = urllib.parse.urlencode(
        {"text": chunk, "language": _LANGUAGE}
    ).encode("utf-8")
    request = urllib.request.Request(
        f"{server_url}/v2/check",
        data=payload,
        headers={"Content-Type": "application/x-www-form-urlencoded"},
    )
    with urllib.request.urlopen(request, timeout=_REQUEST_TIMEOUT_S) as resp:
        data = json.loads(resp.read().decode("utf-8"))
    return data.get("matches", [])


def check_text(text: str, *, server_url: str = DEFAULT_SERVER_URL) -> list[dict]:
    """Submete o texto (em chunks) ao servidor local; offsets reajustados
    para o texto completo."""
    matches: list[dict] = []
    search_pos = 0
    for chunk in chunk_text(text, size=_LOCAL_SAFE_CHUNK):
        # Cada chunk é substring literal do texto: localiza a posição real
        # para reajustar offsets (robusto a separadores variados).
        offset_base = text.find(chunk, search_pos)
        if offset_base == -1:
            raise RuntimeError("chunk não encontrado no texto original")
        search_pos = offset_base + len(chunk)
        for match in _check_chunk(chunk, server_url):
            match = dict(match)
            match["offset"] = match["offset"] + offset_base
            matches.append(match)
    return matches


def apply_suggestions(text: str, matches: list[dict]) -> tuple[str, int]:
    """Gera a PROPOSTA: aplica a 1ª sugestão de cada ocorrência não sobreposta.

    Função pura e determinística. Ocorrências sem sugestão de troca são
    puladas (ficam só no relatório JSON, para leitura humana).
    """
    applied = 0
    cursor = 0
    parts: list[str] = []
    for match in sorted(matches, key=lambda m: (m["offset"], m["length"])):
        start, length = match["offset"], match["length"]
        replacements = match.get("replacements") or []
        # Pula sobreposição com correção já aplicada e matches sem sugestão.
        if start < cursor or not replacements:
            continue
        if start + length > len(text):
            continue
        parts.append(text[cursor:start])
        parts.append(replacements[0]["value"])
        cursor = start + length
        applied += 1
    parts.append(text[cursor:])
    return "".join(parts), applied


def review_file(
    cleaned_path: Path,
    *,
    server_url: str = DEFAULT_SERVER_URL,
    out_dir: Path | None = None,
) -> LocalCheckResult:
    """Fluxo completo para um cleaned.md: checa, propõe e grava artefatos."""
    text = cleaned_path.read_text(encoding="utf-8")
    out_dir = out_dir or (cleaned_path.parent / "languagetool")
    out_dir.mkdir(parents=True, exist_ok=True)

    matches = check_text(text, server_url=server_url)
    corrected, applied = apply_suggestions(text, matches)
    changed = corrected != text

    suggestions_path = out_dir / "lt-local-suggestions.json"
    suggestions_path.write_text(
        json.dumps(
            {
                "generated_at": datetime.now(timezone.utc).isoformat(),
                "engine": "languagetool-local",
                "language": _LANGUAGE,
                "source": str(cleaned_path),
                "source_sha256": sha256_text(text),
                "total_matches": len(matches),
                "applied_in_proposal": applied,
                "matches": matches,
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )

    corrected_path = out_dir / "lt-local-corrected.md"
    corrected_path.write_text(corrected, encoding="utf-8")

    diff_path = out_dir / "lt-local-changes.diff"
    diff_path.write_text(build_diff(text, corrected), encoding="utf-8")

    return LocalCheckResult(
        matches=matches,
        corrected_text=corrected,
        changed=changed,
        suggestions_path=suggestions_path,
        corrected_path=corrected_path,
        diff_path=diff_path,
        stats={"total_matches": len(matches), "applied_in_proposal": applied},
    )
