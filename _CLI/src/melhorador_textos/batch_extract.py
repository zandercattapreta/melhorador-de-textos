# ==============================================================================
# SCRIPT: batch_extract.py
# DESCRIÇÃO: Orquestra a extração de todos os PDFs de _originais/ em sequência
# CHAMADO POR: cli.py (comando batch-extract), melhorar.sh
# DEPENDÊNCIAS: pypdf; módulos internos cli/metadata/languagetool_review
# CONTRATO (RESPOSTA ESPERADA): BatchReport; grava _output/BATCH_REPORT.json
# ==============================================================================
"""Processamento em lote de múltiplos PDFs.

Fluxo por livro:
1. Descobre PDFs em `_originais/` (não recursivo).
2. Extrai metadados da ficha catalográfica (autor/título/ISBN → slug);
   fallback: nome do arquivo.
3. Roda o pipeline `run_extract` (extração → limpeza → estrutura → report).
4. Gera o pacote de revisão LanguageTool (prepare-lt) do cleaned.md.

Regras do projeto respeitadas aqui:
- Fail-fast: sem retry por padrão (pipeline é determinístico); na primeira
  falha persistente o batch PARA e salva checkpoint.
- Gate APAE: por padrão processa só a faixa-amostra (1–50). O livro inteiro
  exige a flag explícita --full — a digitação da flag é a autorização.
- Saídas sempre em `_output/` e temporários em `_temp/` (fixos, local-only).
"""

from __future__ import annotations

import json
import logging
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path

from pypdf import PdfReader

from .cli import run_extract
from .extraction import parse_page_range
from .languagetool_local import ensure_server, review_file
from .languagetool_review import prepare_package
from .metadata import extract_pdf_metadata

logger = logging.getLogger(__name__)

# Configura logging simples se o chamador não configurou.
if not logger.handlers:
    handler = logging.StreamHandler()
    handler.setFormatter(logging.Formatter("%(message)s"))
    logger.addHandler(handler)
    logger.setLevel(logging.INFO)

# Faixa-amostra padrão por livro. Livro inteiro só com --full (gate APAE).
DEFAULT_SAMPLE_PAGES = "1-50"

# Diretórios fixos do contrato do projeto (local-only, ver .gitignore).
_INPUT_DIR_DEFAULT = Path("_originais")
_OUTPUT_ROOT = Path("_output")
_TEMP_ROOT = Path("_temp")


# ==============================================================================
# MODELOS DE DADOS
# ==============================================================================

@dataclass
class BookRecord:
    """Estado de um livro no processamento em lote.

    Caminhos guardados como str para serialização JSON direta (asdict).
    """

    name: str
    slug: str
    input_pdf: str
    pages: tuple[int, int]  # (início, fim), 1-indexado

    status: str = "pending"  # pending | processing | success | failed | skipped
    attempts: int = 0
    error: str | None = None
    output_dir: str | None = None
    cleaned_sha256: str | None = None
    lt_local: str | None = None  # done | skipped | error: <msg>

    metadata_source: str | None = None  # ficha_catalografica | filename
    metadata_confidence: float | None = None


@dataclass
class BatchReport:
    """Relatório final de um batch de extração."""

    batch_id: str
    timestamp: str
    input_dir: str
    output_dir: str

    total_books: int = 0
    processed: int = 0
    succeeded: int = 0
    failed: int = 0
    skipped: int = 0

    books: list[BookRecord] = field(default_factory=list)

    def to_dict(self) -> dict:
        """Serializa para JSON (BookRecord já usa tipos primitivos)."""
        return asdict(self)


# ==============================================================================
# DESCOBERTA E FAIXA DE PÁGINAS
# ==============================================================================

def discover_pdfs(input_dir: Path) -> list[Path]:
    """Lista os PDFs de input_dir (não recursivo), em ordem estável."""
    if not input_dir.exists():
        raise FileNotFoundError(f"Diretório não encontrado: {input_dir}")
    return sorted(input_dir.glob("*.pdf"))


def get_pdf_page_count(pdf_path: Path) -> int:
    """Número total de páginas do PDF via pypdf."""
    reader = PdfReader(str(pdf_path))
    return len(reader.pages)


def resolve_pages(spec: str | None, total: int, *, full: bool = False) -> list[int]:
    """Resolve a faixa de páginas de um livro.

    - full=True → livro inteiro (1..total). Uso condicionado ao gate APAE:
      o chamador só ativa via flag explícita --full.
    - Senão, usa a faixa pedida (ou DEFAULT_SAMPLE_PAGES) recortada ao total,
      para que livros menores que a amostra não estourem o recorte do PDF.
    """
    if total < 1:
        raise ValueError(f"PDF sem páginas (total={total})")
    if full:
        return list(range(1, total + 1))

    pages = parse_page_range(spec or DEFAULT_SAMPLE_PAGES)
    clipped = [p for p in pages if p <= total]
    if not clipped:
        raise ValueError(
            f"Faixa {spec!r} começa além do total de páginas ({total})"
        )
    return clipped


# ==============================================================================
# PROCESSAMENTO POR LIVRO
# ==============================================================================

def process_book(
    book: BookRecord,
    pages: list[int],
    *,
    languages: str = "por+eng",
    max_retries: int = 1,
    lt_local: bool = False,
) -> bool:
    """Roda o pipeline completo de um livro; atualiza o BookRecord.

    max_retries=1 é o padrão fail-fast do projeto: o pipeline é determinístico,
    repetir a mesma entrada tende a repetir a mesma falha. Valores maiores só
    fazem sentido para falhas transitórias de ambiente (disco/subprocesso).
    """
    for attempt in range(1, max_retries + 1):
        book.attempts = attempt
        book.status = "processing"
        try:
            logger.info("  [%s] tentativa %d/%d", book.slug, attempt, max_retries)

            # Pipeline canônico: mesma função do comando `extract` (report.json,
            # hashes e avisos inclusos) — nada duplicado aqui.
            report = run_extract(
                Path(book.input_pdf),
                pages,
                book.slug,
                languages=languages,
            )

            # Pacote LanguageTool pronto para a revisão humana (Premium).
            cleaned_path = Path(report["outputs"]["cleaned"])
            prepare_package(
                cleaned_path.read_text(encoding="utf-8"),
                cleaned_path.parent / "languagetool",
            )

            # Camada automática do híbrido: LT local (proposta + diff).
            # Falha aqui não derruba o livro — a extração já está íntegra e
            # o check-lt pode ser rodado depois, isolado.
            if lt_local:
                try:
                    lt = review_file(cleaned_path)
                    book.lt_local = "done"
                    logger.info(
                        "  [%s] lt-local: %d ocorrência(s), %d na proposta",
                        book.slug,
                        lt.stats["total_matches"],
                        lt.stats["applied_in_proposal"],
                    )
                except Exception as lt_exc:  # noqa: BLE001 — etapa opcional
                    book.lt_local = f"error: {str(lt_exc)[:120]}"
                    logger.warning(
                        "  [%s] lt-local falhou (%s) — siga com check-lt manual",
                        book.slug,
                        book.lt_local,
                    )
            else:
                book.lt_local = "skipped"

            book.output_dir = str(cleaned_path.parent)
            book.cleaned_sha256 = report["cleaned_sha256"]
            book.status = "success"
            return True
        except Exception as exc:  # noqa: BLE001 — registrado no relatório
            book.error = str(exc)[:200]
            logger.error("  [%s] falha: %s", book.slug, book.error)

    book.status = "failed"
    return False


# ==============================================================================
# CHECKPOINT
# ==============================================================================

def load_completed_slugs(checkpoint_path: Path) -> set[str]:
    """Slugs já concluídos com sucesso no checkpoint anterior (se houver)."""
    if not checkpoint_path.exists():
        return set()
    try:
        data = json.loads(checkpoint_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        logger.warning("[batch] checkpoint ilegível (%s) — ignorando", exc)
        return set()
    return {
        book["slug"]
        for book in data.get("books", [])
        if book.get("status") == "success"
    }


def save_checkpoint(checkpoint_path: Path, report: BatchReport) -> None:
    """Salva o estado atual para permitir --resume."""
    checkpoint_path.parent.mkdir(parents=True, exist_ok=True)
    checkpoint_path.write_text(
        json.dumps(report.to_dict(), ensure_ascii=False, indent=2),
        encoding="utf-8",
    )


# ==============================================================================
# ORQUESTRAÇÃO
# ==============================================================================

def batch_extract(
    input_dir: Path = _INPUT_DIR_DEFAULT,
    *,
    pages_spec: str | None = None,
    full: bool = False,
    languages: str = "por+eng",
    max_retries: int = 1,
    resume: bool = False,
    lt_local: bool = True,
) -> BatchReport:
    """Processa todos os PDFs de input_dir sequencialmente.

    Na primeira falha persistente: PARA, salva checkpoint e retorna (fail-fast).
    Com resume=True, pula livros já concluídos no checkpoint anterior.
    """
    checkpoint_path = _TEMP_ROOT / "batch-checkpoint.json"
    report = BatchReport(
        batch_id=datetime.now(timezone.utc).strftime("%Y-%m-%d_%H-%M-%S"),
        timestamp=datetime.now(timezone.utc).isoformat(),
        input_dir=str(input_dir),
        output_dir=str(_OUTPUT_ROOT),
    )

    pdfs = discover_pdfs(input_dir)
    report.total_books = len(pdfs)
    logger.info("[batch] %d PDF(s) em %s", len(pdfs), input_dir)
    if not pdfs:
        logger.warning("[batch] nenhum PDF encontrado — nada a fazer")
        return report

    completed = load_completed_slugs(checkpoint_path) if resume else set()
    if completed:
        logger.info("[batch] resume: %d livro(s) já concluído(s)", len(completed))

    # Sobe o servidor LT local uma vez para o batch inteiro; sem ele, a
    # etapa automática é pulada com aviso (extração segue normalmente).
    if lt_local and not ensure_server():
        logger.warning(
            "[batch] LanguageTool local indisponível — etapa automática pulada "
            "(instale com `brew install languagetool`)"
        )
        lt_local = False

    for pdf_path in pdfs:
        meta = extract_pdf_metadata(pdf_path)
        total_pages = get_pdf_page_count(pdf_path)
        book = BookRecord(
            name=meta["slug"],
            slug=meta["slug"],
            input_pdf=str(pdf_path),
            pages=(1, total_pages),
            metadata_source=meta["source"],
            metadata_confidence=meta.get("confidence"),
        )

        # Pula livros já concluídos (resume de checkpoint).
        if book.slug in completed:
            book.status = "skipped"
            report.books.append(book)
            report.skipped += 1
            logger.info("[%s] já concluído (checkpoint) — pulando", book.slug)
            continue

        pages = resolve_pages(pages_spec, total_pages, full=full)
        book.pages = (pages[0], pages[-1])
        logger.info(
            "\n[%s] iniciando — páginas %d–%d de %d%s",
            book.slug,
            pages[0],
            pages[-1],
            total_pages,
            " (LIVRO INTEIRO)" if full else " (amostra)",
        )

        success = process_book(
            book,
            pages,
            languages=languages,
            max_retries=max_retries,
            lt_local=lt_local,
        )
        report.books.append(book)
        report.processed += 1

        if success:
            report.succeeded += 1
            save_checkpoint(checkpoint_path, report)
        else:
            # Fail-fast: interrompe o batch na primeira falha persistente.
            report.failed += 1
            save_checkpoint(checkpoint_path, report)
            logger.error(
                "[batch] PARADO: %s falhou. Corrija e rode com --resume.",
                book.slug,
            )
            break

    # Relatório final (parcial ou completo) sempre gravado em _output/.
    report_path = _OUTPUT_ROOT / "BATCH_REPORT.json"
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(
        json.dumps(report.to_dict(), ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    logger.info(
        "\n[batch] %d sucesso / %d falha / %d pulado — relatório: %s",
        report.succeeded,
        report.failed,
        report.skipped,
        report_path,
    )
    return report
