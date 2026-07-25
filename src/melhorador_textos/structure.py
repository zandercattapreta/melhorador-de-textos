# ==============================================================================
# SCRIPT: structure.py
# DESCRIÇÃO: Detecta títulos H1–H4 e blocos de SUMÁRIO (heurísticas, sem IA)
# CHAMADO POR: melhorador_textos.cli (após cleanup), tests/test_structure.py
# DEPENDÊNCIAS: re, unicodedata
# CONTRATO (RESPOSTA ESPERADA): StructureResult { text: str, stats: dict }
# ==============================================================================
"""Estruturação Markdown determinística.

Não usa IA. Classifica parágrafos já limpos em headings (#–####) ou prosa,
com tratamento especial de SUMÁRIO/ÍNDICE.
"""

from __future__ import annotations

import re
import unicodedata
from dataclasses import dataclass, field

# Limite de caracteres para considerar um parágrafo "título curto".
# Acima disso, mesmo começando com "1.", trata-se como prosa/lista longa.
_HEADING_MAX_CHARS = 90

# Prosa/título numerado: "1. Título" ou "1.2 Sub" / "1.2.3 Detalhe".
_NUMBERED_RE = re.compile(
    r"^(?:(?P<num_multi>\d+(?:\.\d+){1,3})\s+|(?P<num_simple>\d+)\.\s+)(?P<title>.+)$",
    re.UNICODE,
)

# Entrada de sumário: "Título ........ 42" ou "Título … 42"
_TOC_ENTRY_RE = re.compile(
    r"^.+?[\.…]{2,}\s*\d{1,4}\s*$",
    re.UNICODE,
)

# Palavras que abrem bloco de sumário / índice.
_TOC_MARKERS = {
    "sumario",
    "sumário",
    "indice",
    "índice",
    "conteudo",
    "conteúdo",
    "table of contents",
}

# Fragmentos típicos de página de créditos/colofão — não são títulos de capítulo.
_COLOPHON_FOLDS = (
    "deposito legal",
    "paginacao",
    "impressao",
    "acabamento",
    "design de capa",
    "isbn",
    "copyright",
    "todos os direitos",
    "titulo original",
)


def _looks_like_colophon(text: str) -> bool:
    folded = _fold(text)
    return any(marker in folded for marker in _COLOPHON_FOLDS)


# Títulos de seção sem numeração, curtos e conhecidos (PT editorial).
_NAMED_H1 = {
    "preambulo",
    "preâmbulo",
    "introducao",
    "introdução",
    "conclusao",
    "conclusão",
    "epilogo",
    "epílogo",
    "anexo",
    "apendice",
    "apêndice",
    "bibliografia",
    "agradecimentos",
    "nota do autor",
    "nota do tradutor",
}

# Correção OCR estritamente limitada a prefixo de heading romano inválido.
# Em fontes serifadas, "II." é frequentemente reconhecido como "IL".
_ROMAN_II_OCR_RE = re.compile(r"^IL\s*[.\-–—]\s*", re.UNICODE)


@dataclass
class StructureResult:
    """Texto com headings Markdown + métricas."""

    text: str
    stats: dict = field(default_factory=dict)


def _fold(text: str) -> str:
    """Normaliza para comparação (sem acento, lower)."""
    decomposed = unicodedata.normalize("NFD", text.strip())
    return "".join(
        ch for ch in decomposed if unicodedata.category(ch) != "Mn"
    ).casefold()


def _letter_ratio_upper(text: str) -> float:
    """Proporção de letras maiúsculas entre as letras do texto."""
    letters = [ch for ch in text if ch.isalpha()]
    if not letters:
        return 0.0
    return sum(1 for ch in letters if ch.isupper()) / len(letters)


def _is_toc_marker(para: str) -> bool:
    folded = _fold(para)
    # Aceita "SUMÁRIO", "Índice geral", etc. se a palavra-chave for o núcleo.
    if folded in _TOC_MARKERS:
        return True
    return any(folded == m or folded.startswith(m + " ") for m in _TOC_MARKERS)


def _is_named_h1(para: str) -> bool:
    return _fold(para) in _NAMED_H1


def _numbered_level(num: str) -> int:
    """Converte '1' → 2, '1.2' → 3, '1.2.3' → 4 (H2–H4)."""
    depth = num.count(".") + 1
    # depth 1 → H2, 2 → H3, 3+ → H4
    return min(depth + 1, 4)


def _is_title_case_subheading(
    text: str, *, previous: str | None, following: str | None
) -> bool:
    """Detecta subtítulo curto isolado entre blocos de prosa.

    A decisão usa posição, não semântica: candidato curto sem pontuação final,
    seguido por prosa longa e precedido por heading ou prosa longa.
    """
    if not (8 <= len(text) <= 50):
        return False
    if not text[0].isupper() or text[-1] in ".!?:;,":
        return False
    if any(char.isdigit() for char in text) or _looks_like_colophon(text):
        return False
    words = text.split()
    if not (2 <= len(words) <= 8):
        return False
    if following is None or len(following) < 120:
        return False
    if previous is None:
        return False
    return previous.startswith("#") or len(previous) >= 120


def _normalize_heading_ocr(text: str) -> tuple[str, bool]:
    """Corrige confusão OCR comprovável em numeral romano de heading."""
    normalized, count = _ROMAN_II_OCR_RE.subn("II. — ", text, count=1)
    return normalized, bool(count)


def _classify_paragraph(para: str, *, in_toc: bool) -> tuple[int | None, bool]:
    """Classifica um parágrafo.

    Retorna (nível_markdown | None, ainda_em_toc).
    Nível 1–4 → heading; None → prosa (ou linha de sumário sem #).
    """
    text = para.strip()
    if not text:
        return None, in_toc

    # --- Bloco SUMÁRIO / ÍNDICE ---
    if _is_toc_marker(text):
        return 1, True

    if in_toc:
        # Sai do modo TOC ao encontrar prosa longa sem padrão de entrada.
        if _TOC_ENTRY_RE.match(text) or len(text) <= _HEADING_MAX_CHARS:
            # Mantém como linha de sumário (sem #); caller prefixa com "- ".
            return 0, True  # 0 = toc entry (lista)
        return None, False

    # --- H1 por nome conhecido (Preâmbulo, Introdução…) ---
    if len(text) <= _HEADING_MAX_CHARS and _is_named_h1(text):
        return 1, False

    # --- H1 por caixa alta (título de parte/capítulo) ---
    if (
        len(text) <= 70
        and len(text) >= 8
        and _letter_ratio_upper(text) >= 0.75
        and not text.endswith(".")
        and ";" not in text  # OCR costuma colar várias linhas com ";"
        and not _looks_like_colophon(text)
    ):
        return 1, False

    # --- H2–H4 numerados curtos ---
    match = _NUMBERED_RE.match(text)
    if match and len(text) <= _HEADING_MAX_CHARS:
        title = match.group("title")
        num = match.group("num_multi") or match.group("num_simple")
        # Evita falsos positivos tipo "1. 2773" — título precisa de letras.
        if num and any(ch.isalpha() for ch in title):
            return _numbered_level(num), False

    return None, in_toc


def apply_structure(text: str) -> StructureResult:
    """Aplica headings Markdown aos parágrafos limpos.

    Ordem: cada bloco separado por linha em branco é classificado. Entradas
    de sumário viram itens de lista (`- …`). Prosa numerada longa não vira
    heading (limite _HEADING_MAX_CHARS).
    """
    paragraphs = [p for p in text.split("\n\n")]
    out: list[str] = []
    in_toc = False
    counts = {
        "h1": 0,
        "h2": 0,
        "h3": 0,
        "h4": 0,
        "toc_entries": 0,
        "prose": 0,
        "title_case_headings": 0,
        "heading_ocr_corrections": 0,
    }

    nonempty = [paragraph.strip() for paragraph in paragraphs if paragraph.strip()]
    for index, stripped in enumerate(nonempty):
        previous = out[-1] if out else None
        following = nonempty[index + 1] if index + 1 < len(nonempty) else None

        level, in_toc = _classify_paragraph(stripped, in_toc=in_toc)
        if level is None and not in_toc and _is_title_case_subheading(
            stripped, previous=previous, following=following
        ):
            level = 2
            counts["title_case_headings"] += 1

        if level == 0:
            # Entrada de sumário → lista Markdown.
            out.append(f"- {stripped}")
            counts["toc_entries"] += 1
        elif level is not None:
            normalized, corrected = _normalize_heading_ocr(stripped)
            out.append(f"{'#' * level} {normalized}")
            counts[f"h{level}"] += 1
            counts["heading_ocr_corrections"] += int(corrected)
        else:
            out.append(stripped)
            counts["prose"] += 1

    result_text = "\n\n".join(out) + ("\n" if out else "")
    return StructureResult(text=result_text, stats=counts)
