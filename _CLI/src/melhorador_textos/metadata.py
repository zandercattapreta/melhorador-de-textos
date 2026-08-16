# ==============================================================================
# SCRIPT: metadata.py
# DESCRIÇÃO: Extrai AUTOR, TÍTULO, ISBN das fichas catalográficas de PDFs
# CHAMADO POR: batch_extract.py (descoberta de nomes)
# DEPENDÊNCIAS: pypdf, re, pathlib
# CONTRATO (RESPOSTA ESPERADA): dict { source, author, title, isbn, slug }
# ==============================================================================
"""Extração de metadados de ficha catalográfica (primeiras 10 páginas do PDF).

Estratégia:
1. Extrai texto das primeiras 10 páginas
2. Procura por padrões de ficha (AUTOR, TÍTULO, ISBN)
3. Aplica heurísticas de confiança (ver CONFIDENCE_MATRIX)
4. Normaliza slug: autor-titulo (sem ISBN se não encontrado)
5. Fallback: nome do PDF

APRENDIZADO DE CASOS REAIS:
- Paideia (CIP bem-formada + OCR): author+title+isbn detectam → confiança 0.85
- Primeiras Civilizações (OCR puro sujo): apenas ISBN detecta → fallback ao filename
- Padrão "author sem data" é frágil e ambíguo → desabilitado

Conclusão: ISBN é o padrão mais confiável (0.95). Autor+data é confiável (0.9).
Título é moderado (0.8). Autor sem data é frágil (0.3) — evitar.
"""

from __future__ import annotations

import re
from pathlib import Path

from pypdf import PdfReader

# ==============================================================================
# CONFIDENCE MATRIX — Baseada em casos reais testados
# ==============================================================================
CONFIDENCE_SCORES = {
    "author_with_date": 0.9,      # Padrão CIP bem-formado (Paideia)
    "author_without_date": 0.3,   # Frágil, ambíguo em OCR sujo (Primeiras Civilizações)
    "title_with_colon": 0.8,      # Funciona bem quando há dois-pontos
    "isbn": 0.95,                 # Padrão mais confiável
}

DECISION_HEURISTICS = {
    # Se autor + título + ISBN detectados
    "all_fields": {
        "source": "ficha_catalografica",
        "confidence": 0.85,  # média de autor(0.9) + título(0.8) + isbn(0.95)
        "action": "use_metadata",
    },
    # Se apenas ISBN e título (sem autor)
    "isbn_title_no_author": {
        "source": "ficha_catalografica",
        "confidence": 0.65,  # isbn(0.95) + título(0.8) - autor
        "action": "fallback_to_filename",  # título sozinho é ambíguo
        "reason": "author not found",
    },
    # Se apenas ISBN e autor (sem título)
    "isbn_author_no_title": {
        "source": "ficha_catalografica",
        "confidence": 0.73,  # isbn(0.95) + author(0.9) > título
        "action": "use_metadata",  # slug = "{author}-{isbn}"
    },
    # Se nenhum campo detectado
    "no_fields": {
        "source": "filename",
        "confidence": 0.55,  # fallback genérico
        "action": "fallback_to_filename",
        "reason": "ocr_text_too_noisy",
    },
}


def extract_pdf_metadata(pdf_path: Path) -> dict:
    """
    Extrai AUTOR, TÍTULO, ISBN das primeiras 10 páginas.
    Aplica heurísticas de confiança (baseadas em casos reais).
    Retorna slug normalizado: autor-titulo[-isbn].
    Fallback: nome do PDF.
    """
    if not pdf_path.exists():
        raise FileNotFoundError(f"PDF não encontrado: {pdf_path}")

    try:
        # Extrai texto das primeiras 10 páginas
        text = _extract_first_pages(pdf_path, 10)

        author = _parse_author(text)
        title = _parse_title(text)
        isbn = _parse_isbn(text)

        # Aplica heurísticas de decisão
        if author and title and isbn:
            # Caso ideal: todos os campos detectados
            slug = _normalize_slug(author, title, isbn)
            return {
                "source": "ficha_catalografica",
                "author": author,
                "title": title,
                "isbn": isbn,
                "slug": slug,
                "confidence": DECISION_HEURISTICS["all_fields"]["confidence"],
            }

        if isbn and title and not author:
            # Caso débil: apenas ISBN e título (sem autor)
            # Heurística: forçar fallback (título sozinho é ambíguo)
            pass

        if isbn and author and not title:
            # Caso aceitável: ISBN e autor (sem título)
            slug = _normalize_slug(author, None, isbn)
            return {
                "source": "ficha_catalografica",
                "author": author,
                "title": None,
                "isbn": isbn,
                "slug": slug,
                "confidence": DECISION_HEURISTICS["isbn_author_no_title"]["confidence"],
            }

        # Se chegou aqui, não tem confiança mínima: fallback
    except Exception as e:
        # Falha de extração → fallback silencioso
        pass

    # Fallback: nome do PDF
    slug = _normalize_slug_from_filename(pdf_path.stem)
    return {
        "source": "filename",
        "slug": slug,
        "author": None,
        "title": None,
        "isbn": None,
        "confidence": DECISION_HEURISTICS["no_fields"]["confidence"],
    }


def _extract_first_pages(pdf_path: Path, n_pages: int = 10) -> str:
    """Extrai texto das primeiras N páginas do PDF."""
    reader = PdfReader(str(pdf_path))
    pages = reader.pages[: min(n_pages, len(reader.pages))]
    text_parts = []
    for page in pages:
        text = page.extract_text()
        if text:
            text_parts.append(text)
    return "\n".join(text_parts)


def _parse_author(text: str) -> str | None:
    """
    Procura por padrões de autor em ficha catalográfica.
    Padrão preferido: LASTNAME, Firstname(s), YYYY-ZZZZ (com data).
    Padrão 2 (frágil): LASTNAME, Firstname sem data — evitar usar.
    Tolerante com espaços fragmentados (OCR sujo).
    """
    # Padrão 1: "Palavra(s), Palavra(s), YYYY-ZZZZ" (com data)
    # Este é o padrão confiável; CIP bem-formada
    match = re.search(
        r"([A-Za-záéíóúäëïöüâêõã](?:\s?[A-Za-záéíóúäëïöüâêõã])*)"
        r"\s*,\s*"
        r"([A-Za-záéíóúäëïöüâêõã](?:\s?[A-Za-záéíóúäëïöüâêõã])*)"
        r"\s*,?\s*\d{4}-\d{4}",
        text,
    )
    if match:
        last_name = match.group(1).strip()
        first_names = match.group(2).strip()
        first_names = re.sub(r"(\w)\s+([a-záéíóúäëïöüâêõã])", r"\1\2", first_names)
        last_name = re.sub(r"(\w)\s+([a-záéíóúäëïöüâêõã])", r"\1\2", last_name)

        # Validação: descarta se contém lixo (números soltos, linhas quebradas)
        if "\n" not in f"{first_names} {last_name}":
            return f"{first_names} {last_name}"

    # Padrão 2 (FRÁGIL): "LASTNAME, Firstname" sem data
    # NOTA: Desabilitado por padrão — causa muitos falsos positivos em OCR sujo
    # Exemplo: "I - LEVEQUE, Pierre" é ambíguo (pode ser índice, não autor)
    # Decisão: NUNCA usar este padrão; preferir fallback ao filename

    return None


def _parse_title(text: str) -> str | None:
    """
    Procura por título em ficha catalográfica.
    Padrão: "TÍTULO : subtítulo" (com dois-pontos).
    """
    # Busca por padrão "Palavra : descrição" onde Palavra é título
    # (começa com maiúscula, tem comprimento médio 3–50 caracteres)
    match = re.search(
        r"([A-ZÀ-Ÿ][a-záéíóúäëïöüâêõã]{2,}(?:\s+[a-záéíóúäëïöüâêõã]+)*)\s*:\s*([a-záéíóúäëïöüâêõã\s/\-]{5,})",
        text,
    )
    if match:
        title = match.group(1).strip()
        subtitle = match.group(2).strip()[:80]

        # Descarta títulos muito curtos ou metadados
        if len(title) > 3 and "ISBN" not in title.upper() and "copyright" not in title.lower():
            return f"{title} : {subtitle}"

    return None


def _parse_isbn(text: str) -> str | None:
    """
    Procura por ISBN (padrão: 978-XXXXX-XXXXX-X).
    Retorna sem hífens (13 dígitos para ISBN-13, ou 10 para ISBN-10).
    Prefere ISBN-13 (com 978 prefixo).
    """
    # Padrão 1: ISBN-13 com prefixo 978 (mais comum)
    match = re.search(
        r"ISBN\s*978[-\s]?([0-9]{1,5})[-\s]?([0-9]{1,5})[-\s]?([0-9]{1,5})[-\s]?([0-9])",
        text,
        re.IGNORECASE,
    )
    if match:
        isbn = "978" + "".join(match.groups())
        if len(isbn) >= 13:
            return isbn

    # Padrão 2: ISBN sem prefixo (fallback)
    match = re.search(
        r"ISBN\s*[-\s]?([0-9]{1,5})[-\s]?([0-9]{1,5})[-\s]?([0-9]{1,5})[-\s]?([0-9])",
        text,
        re.IGNORECASE,
    )
    if match:
        isbn = "".join(match.groups())
        if len(isbn) >= 10:
            return isbn
    return None


def _normalize_slug(author: str | None, title: str | None, isbn: str | None) -> str:
    """
    Normaliza author-title[-isbn] para slug válido.
    Regras:
    - Minúsculas
    - Sem acentos
    - Espaços e pontos → hífens
    - Máx 80 caracteres
    """
    parts = []

    if author:
        parts.append(_clean_slug_part(author, max_len=30))
    if title:
        parts.append(_clean_slug_part(title, max_len=40))
    if isbn:
        parts.append(isbn[:10])  # Primeiros 10 dígitos

    slug = "-".join(parts)
    return slug[:80]


def _normalize_slug_from_filename(filename: str) -> str:
    """
    Normaliza nome de arquivo para slug válido.
    Exemplo: "Pierre Leveque - As Primeiras..." → "pierre-leveque-as-primeiras-..."
    """
    return _clean_slug_part(filename, max_len=120)


def _clean_slug_part(text: str, max_len: int = 50) -> str:
    """Limpa uma parte do slug: minúsculas, remove acentos, normaliza espaços."""
    # Remove acentos
    import unicodedata

    text = unicodedata.normalize("NFKD", text)
    text = text.encode("ascii", "ignore").decode("ascii")

    # Minúsculas, substitui espaços/pontos por hífen
    text = text.lower()
    text = re.sub(r"[^\w-]", "-", text)
    text = re.sub(r"-+", "-", text)  # múltiplos hífens → um
    text = text.strip("-")

    return text[:max_len]
