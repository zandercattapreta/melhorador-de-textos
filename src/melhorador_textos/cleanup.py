# ==============================================================================
# SCRIPT: cleanup.py
# DESCRIÇÃO: Limpeza determinística de texto OCR/PDF sem inventar conteúdo
# CHAMADO POR: melhorador_textos.cli (comando extract), tests/test_cleanup.py
# DEPENDÊNCIAS: ftfy, re, unicodedata
# CONTRATO (RESPOSTA ESPERADA): CleanupResult { text: str, stats: dict, warnings: list }
# ==============================================================================
"""Camada de limpeza pós-extração.

Regra de ouro do projeto: só estabilizar e reformatar. Nunca completar,
adivinhar ou reescrever o conteúdo do livro. Toda transformação aqui é
determinística e reversível em intenção (não usa modelos generativos).
"""

from __future__ import annotations

import re
import unicodedata
from collections import Counter
from dataclasses import dataclass, field
from difflib import SequenceMatcher

import ftfy

# Caracteres invisíveis/indesejados comuns em extração de PDF e OCR.
# Removidos porque poluem o texto sem carregar significado.
_INVISIBLE_CHARS = {
    "\u200b",  # zero-width space
    "\u200c",  # zero-width non-joiner
    "\u200d",  # zero-width joiner
    "\ufeff",  # BOM / zero-width no-break space
    "\u00ad",  # soft hyphen
    "\u180e",  # mongolian vowel separator
}

# Marcador de página do sidecar do OCRmyPDF/pdftotext no formato "-- N of M --".
_PAGE_MARKER_RE = re.compile(r"^\s*--\s*\d+\s+of\s+\d+\s*--\s*$", re.MULTILINE)

# Número de página isolado (rodapé típico de livro).
_PAGE_NUMBER_RE = re.compile(r"^\d{1,4}$")

# Ruído de 1 caractere no fim da página (aspas órfãs do OCR, etc.).
_EDGE_GARBAGE_RE = re.compile(r"^[^\wÀ-ÿ]$", re.UNICODE)

# Hifenização de quebra de linha: "pala-\nvra" -> "palavra".
# Só juntamos quando há minúscula antes e depois do hífen (padrão de
# justificação tipográfica). O módulo `re` não suporta \p{Ll}, então usamos
# faixas Unicode explícitas para latim acentuado.
_LINE_HYPHEN_RE = re.compile(r"([A-Za-zÀ-ÿ]+)-\n([a-zà-ÿ])")

# Seção numerada colada ao fim do parágrafo anterior pelo reflow/OCR.
_EMBEDDED_NUMBERED_RE = re.compile(
    r"(?<=[.!?»])\s+(?=\d{1,2}\.\s+[A-ZÁÀÂÃÉÊÍÓÔÕÚÇ])"
)

# Barra vertical isolada e sequências gráficas sem valor textual.
_ISOLATED_BAR_RE = re.compile(r"(?<!\S)\|(?!\S)")
_OCR_GARBAGE_LINE_RE = re.compile(r"^\s*[—-]*<[-—<>]+.*$", re.UNICODE)

# Número de página inserido pelo OCR no início da continuação de palavra.
_LEADING_PAGE_NUMBER_RE = re.compile(r"^\s*\d{1,4}\s+(?=[a-zà-ÿ])")


@dataclass
class CleanupResult:
    """Resultado da limpeza com métricas para auditoria."""

    text: str
    stats: dict = field(default_factory=dict)
    warnings: list[str] = field(default_factory=list)


def _fold_line(line: str) -> str:
    """Normaliza linha para comparar cabeçalhos (ignora acento/caixa)."""
    decomposed = unicodedata.normalize("NFD", line.strip())
    without_marks = "".join(
        ch for ch in decomposed if unicodedata.category(ch) != "Mn"
    )
    return without_marks.casefold()


def _is_page_number(line: str) -> bool:
    return bool(_PAGE_NUMBER_RE.match(line.strip()))


def _is_edge_garbage(line: str) -> bool:
    return bool(_EDGE_GARBAGE_RE.match(line.strip()))


def _detect_running_headers(page_lines: list[list[str]]) -> set[str]:
    """Detecta cabeçalhos correntes pela frequência da 1ª linha das páginas.

    Linhas que abrem várias páginas (ex.: título de capítulo / PREÂMBULO)
    são tratadas como cabeçalho, não como conteúdo. Comparação ignora
    acentos/caixa para unir variantes de OCR (PREAMBULO vs PREÂMBULO).
    """
    if len(page_lines) < 2:
        return set()

    first_folds = [_fold_line(lines[0]) for lines in page_lines if lines]
    counts = Counter(first_folds)
    # Exige presença em pelo menos 2 páginas (cabeçalho corrente típico).
    return {fold for fold, n in counts.items() if n >= 2 and fold}


def _printed_page_number(page: str) -> str | None:
    """Obtém o número impresso no rodapé, quando ele está isolado."""
    lines = [line.strip() for line in page.splitlines() if line.strip()]
    if lines and _is_page_number(lines[-1]):
        return lines[-1]
    return None


def _page_noise_score(page: str) -> tuple[int, int]:
    """Pontua ruído OCR; menor é melhor.

    Duplicatas de scan aparecem no PDF com o mesmo número impresso. Escolhemos
    deterministicamente a versão com menos sinais de sobreposição/coluna.
    """
    suspicious = 0
    suspicious += page.count("|") * 3
    suspicious += len(re.findall(r"[a-zà-ÿ][A-ZÁÀÂÃÉÊÍÓÔÕÚÇ]", page)) * 2
    suspicious += len(re.findall(r"(?m)^\s*\d{1,4}\s+[a-zà-ÿ]", page)) * 4
    suspicious += len(re.findall(r"[—-]*<[-—<>]+", page)) * 5
    # Em empate, prefere a extração mais completa.
    return suspicious, -len(page)


def _deduplicate_scanned_pages(pages: list[str]) -> tuple[list[str], int]:
    """Remove scans duplicados pelo número de página e similaridade.

    Só deduplica quando as duas versões compartilham o mesmo número de rodapé
    e têm conteúdo suficientemente parecido. Isso evita eliminar páginas
    legítimas que por acaso terminam no mesmo número dentro do corpo.
    """
    by_number: dict[str, list[int]] = {}
    for index, page in enumerate(pages):
        number = _printed_page_number(page)
        if number is not None:
            by_number.setdefault(number, []).append(index)

    remove: set[int] = set()
    for indexes in by_number.values():
        if len(indexes) < 2:
            continue

        candidates = [index for index in indexes if index not in remove]
        best = min(candidates, key=lambda index: _page_noise_score(pages[index]))
        best_folded = _fold_line(pages[best])
        for index in candidates:
            if index == best:
                continue
            similarity = SequenceMatcher(
                None, best_folded, _fold_line(pages[index]), autojunk=False
            ).ratio()
            if similarity >= 0.55:
                remove.add(index)

    return [page for index, page in enumerate(pages) if index not in remove], len(remove)


def _is_header_line(line: str, headers: set[str]) -> bool:
    """True se a linha (ou 'N Título') for um cabeçalho corrente conhecido."""
    stripped = line.strip()
    if not stripped:
        return False
    if _fold_line(stripped) in headers:
        return True
    # Número de página colado ao cabeçalho: "23 As PRIMEIRAS CIVILIZAÇÕES"
    match = re.match(r"^(\d{1,4})\s+(.+)$", stripped)
    if match and _fold_line(match.group(2)) in headers:
        return True
    return False


def _strip_page_chrome(
    text: str, *, drop_leading_pages: int = 0
) -> tuple[str, dict]:
    """Remove cabeçalhos correntes, números de página e ruído de borda.

    Usa o form-feed (\\f) do sidecar do OCRmyPDF para processar página a
    página. Precisa rodar ANTES de apagar caracteres de controle, senão as
    bordas de página se misturam com o texto. Preserva linhas em branco
    internas (separadores de parágrafo).
    """
    # Sem separador de página: remove só números isolados (não inventa layout).
    if "\f" not in text:
        kept: list[str] = []
        removed_numbers = 0
        for line in text.splitlines():
            if _is_page_number(line):
                removed_numbers += 1
                continue
            kept.append(line)
        return "\n".join(kept), {
            "headers_removed": 0,
            "page_numbers_removed": removed_numbers,
            "edge_garbage_removed": 0,
            "pages_processed": 0,
            "leading_pages_dropped": 0,
            "duplicate_pages_removed": 0,
        }

    raw_pages = text.split("\f")
    original_page_count = len(raw_pages)
    if drop_leading_pages < 0:
        raise ValueError("drop_leading_pages não pode ser negativo")
    dropped = min(drop_leading_pages, len(raw_pages))
    raw_pages = raw_pages[dropped:]
    raw_pages, duplicates_removed = _deduplicate_scanned_pages(raw_pages)
    # Primeira passagem: só as linhas não-vazias do topo, para detectar cabeçalhos.
    first_nonempty: list[list[str]] = []
    for page in raw_pages:
        nonempty = [ln.strip() for ln in page.splitlines() if ln.strip()]
        first_nonempty.append(nonempty)

    headers = _detect_running_headers(first_nonempty)
    headers_removed = 0
    page_numbers_removed = 0
    edge_garbage_removed = 0
    cleaned_pages: list[str] = []

    for page in raw_pages:
        # Mantém vazias para não destruir parágrafos do OCR.
        lines = [ln.rstrip() for ln in page.splitlines()]

        # Remove glifo/linha decorativa ilegível antes de juntar parágrafos.
        lines = [line for line in lines if not _OCR_GARBAGE_LINE_RE.match(line)]

        def nonempty_idxs() -> list[int]:
            return [i for i, ln in enumerate(lines) if ln.strip()]

        # Remove cabeçalhos no topo.
        while True:
            idxs = nonempty_idxs()
            if not idxs:
                break
            first = idxs[0]
            if _is_header_line(lines[first], headers):
                lines.pop(first)
                headers_removed += 1
                continue
            # Fragmento curto antes de um cabeçalho conhecido é sujeira da
            # borda/verso da página, não continuação do corpo.
            if (
                len(idxs) >= 2
                and len(lines[first].strip()) <= 3
                and _is_header_line(lines[idxs[1]], headers)
            ):
                lines.pop(first)
                edge_garbage_removed += 1
                continue
            break

        # Remove rodapé: números de página e ruído curto.
        while True:
            idxs = nonempty_idxs()
            if not idxs:
                break
            last = idxs[-1]
            if _is_page_number(lines[last]):
                lines.pop(last)
                page_numbers_removed += 1
                continue
            if _is_edge_garbage(lines[last]):
                lines.pop(last)
                edge_garbage_removed += 1
                continue
            break

        page_text = "\n".join(lines).strip()
        if page_text:
            cleaned_pages.append(page_text)

    # Junta páginas com \\n\\n: o reflow trata cada bloco; frases cortadas
    # no fim da página ainda se reconectam via hifenização + reflow interno.
    # Se o parágrafo atravessa a página sem linha em branco, unimos com \\n.
    joined = "\n".join(cleaned_pages)

    # Passe residual: linhas inteiras que ainda são só cabeçalho / N+cabeçalho.
    residual: list[str] = []
    for line in joined.splitlines():
        if line.strip() and _is_header_line(line, headers):
            headers_removed += 1
            continue
        if _is_page_number(line):
            page_numbers_removed += 1
            continue
        residual.append(line)

    return "\n".join(residual), {
        "headers_removed": headers_removed,
        "page_numbers_removed": page_numbers_removed,
        "edge_garbage_removed": edge_garbage_removed,
        "pages_processed": original_page_count,
        "leading_pages_dropped": dropped,
        "duplicate_pages_removed": duplicates_removed,
    }


def _strip_invisibles(text: str) -> tuple[str, int]:
    """Remove caracteres invisíveis. Retorna texto e contagem removida."""
    removed = 0
    out_chars: list[str] = []
    for ch in text:
        if ch in _INVISIBLE_CHARS:
            removed += 1
            continue
        # Remove caracteres de controle (exceto tab e newline), que quebram texto.
        if unicodedata.category(ch) == "Cc" and ch not in ("\n", "\t"):
            removed += 1
            continue
        out_chars.append(ch)
    return "".join(out_chars), removed


def _dehyphenate(text: str) -> tuple[str, int]:
    """Junta palavras quebradas por hífen no fim da linha.

    Conservador: só age quando há minúscula antes e depois do hífen, evitando
    juntar nomes próprios, siglas ou hifens compostos legítimos.
    """
    count = 0

    def _join(match: re.Match) -> str:
        nonlocal count
        count += 1
        return match.group(1) + match.group(2)

    new_text = _LINE_HYPHEN_RE.sub(_join, text)
    return new_text, count


def _strip_inline_ocr_noise(text: str) -> tuple[str, dict]:
    """Remove ruído estrutural inequívoco sem tentar reconstruir palavras."""
    text, bars_removed = _ISOLATED_BAR_RE.subn("", text)
    text, embedded_sections_split = _EMBEDDED_NUMBERED_RE.subn("\n\n", text)

    # Quando um número de rodapé abre a continuação de uma palavra, removemos
    # apenas o número. Não completamos a palavra: revisão humana permanece
    # necessária se a fonte não trouxer outra cópia limpa.
    lines: list[str] = []
    inline_page_numbers_removed = 0
    garbage_lines_removed = 0
    previous_ended_hyphen = False
    for line in text.splitlines():
        if _OCR_GARBAGE_LINE_RE.match(line):
            garbage_lines_removed += 1
            continue
        if previous_ended_hyphen:
            line, removed = _LEADING_PAGE_NUMBER_RE.subn("", line, count=1)
            inline_page_numbers_removed += removed
        lines.append(line)
        previous_ended_hyphen = line.rstrip().endswith("-")

    return "\n".join(lines), {
        "isolated_bars_removed": bars_removed,
        "embedded_sections_split": embedded_sections_split,
        "inline_page_numbers_removed": inline_page_numbers_removed,
        "garbage_lines_removed": garbage_lines_removed,
    }


def _normalize_whitespace(text: str) -> str:
    """Normaliza espaços e linhas em branco preservando parágrafos.

    - Colapsa espaços/tabs repetidos em um único espaço.
    - Remove espaços no fim de cada linha.
    - Colapsa 3+ quebras de linha em no máximo 2 (separador de parágrafo).
    """
    # Espaços horizontais repetidos -> um espaço.
    text = re.sub(r"[ \t]+", " ", text)
    # Espaço antes de quebra de linha.
    text = re.sub(r"[ \t]+\n", "\n", text)
    # Espaço no início de linha.
    text = re.sub(r"\n[ \t]+", "\n", text)
    # 3+ quebras -> 2 (um parágrafo em branco).
    text = re.sub(r"\n{3,}", "\n\n", text)
    return text.strip() + "\n"


def _reflow_paragraphs(text: str) -> str:
    """Junta linhas soltas de um mesmo parágrafo em uma linha contínua.

    OCR e PDF quebram parágrafos em várias linhas por causa da largura da
    página. Unimos linhas consecutivas de texto, mantendo a quebra dupla
    (parágrafo) como separador.
    """
    paragraphs = text.split("\n\n")
    reflowed: list[str] = []
    for para in paragraphs:
        lines = [ln.strip() for ln in para.split("\n") if ln.strip()]
        if not lines:
            continue
        # Une as linhas do parágrafo com espaço simples.
        reflowed.append(" ".join(lines))
    return "\n\n".join(reflowed) + "\n"


def clean_text(
    text: str, *, reflow: bool = True, drop_leading_pages: int = 0
) -> CleanupResult:
    """Aplica o pipeline determinístico de limpeza.

    Ordem importa: encoding → cabeçalhos/rodapés (com \\f) → invisíveis →
    hifenização → reflow → espaços.
    """
    warnings: list[str] = []
    original_len = len(text)

    # 1. Conserta mojibake e sequências Unicode corrompidas.
    fixed = ftfy.fix_text(text)

    # 2. Normalização Unicode NFC (forma canônica composta, boa p/ PT-BR).
    fixed = unicodedata.normalize("NFC", fixed)

    # 3. Remove marcadores de página do sidecar no formato "-- N of M --".
    fixed, n_markers = _PAGE_MARKER_RE.subn("", fixed)

    # 4. Remove cabeçalhos correntes, números de página e ruído de borda.
    #    Usa \\f do OCR; deve ocorrer ANTES de apagar controles.
    fixed, chrome_stats = _strip_page_chrome(
        fixed, drop_leading_pages=drop_leading_pages
    )

    # 5. Remove caracteres invisíveis / de controle (inclui \\f remanescente).
    fixed, n_invisible = _strip_invisibles(fixed)

    # 6. Remove barras isoladas, números de rodapé colados e separa seções.
    fixed, inline_stats = _strip_inline_ocr_noise(fixed)

    # 7. Junta hifenização de quebra de linha antes do reflow.
    fixed, n_hyphens = _dehyphenate(fixed)

    # 8. Reflow de parágrafos (opcional).
    if reflow:
        fixed = _reflow_paragraphs(fixed)

    # 9. Normaliza espaços em branco.
    fixed = _normalize_whitespace(fixed)

    # Detecta caracteres de substituição remanescentes (OCR duvidoso).
    replacement_count = fixed.count("\ufffd")
    if replacement_count:
        warnings.append(
            f"{replacement_count} caractere(s) de substituição (\ufffd) — revisar OCR"
        )

    stats = {
        "chars_in": original_len,
        "chars_out": len(fixed),
        "page_markers_removed": n_markers,
        "headers_removed": chrome_stats["headers_removed"],
        "page_numbers_removed": chrome_stats["page_numbers_removed"],
        "edge_garbage_removed": chrome_stats["edge_garbage_removed"],
        "pages_processed": chrome_stats["pages_processed"],
        "leading_pages_dropped": chrome_stats["leading_pages_dropped"],
        "duplicate_pages_removed": chrome_stats["duplicate_pages_removed"],
        "isolated_bars_removed": inline_stats["isolated_bars_removed"],
        "embedded_sections_split": inline_stats["embedded_sections_split"],
        "inline_page_numbers_removed": inline_stats[
            "inline_page_numbers_removed"
        ],
        "garbage_lines_removed": inline_stats["garbage_lines_removed"],
        "invisible_removed": n_invisible,
        "hyphenations_joined": n_hyphens,
        "replacement_chars": replacement_count,
    }
    return CleanupResult(text=fixed, stats=stats, warnings=warnings)
