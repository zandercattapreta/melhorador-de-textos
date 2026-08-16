# ==============================================================================
# SCRIPT: extraction.py
# DESCRIÇÃO: Extrai texto de um PDF por faixa de páginas (texto nativo ou OCR)
# CHAMADO POR: melhorador_textos.cli (comando extract)
# DEPENDÊNCIAS: pypdf, ocrmypdf (subprocess), subprocess, shutil
# CONTRATO (RESPOSTA ESPERADA): ExtractionResult { raw_text, engine, pages, meta }
# ==============================================================================
"""Extração de texto de PDFs.

Estratégia por amostra:
1. Recorta a faixa de páginas pedida para um PDF menor (evita OCR do livro todo).
2. Verifica se há camada de texto nativa suficiente.
3. Se houver, usa pypdf. Caso contrário, roda OCRmyPDF forçando OCR.
"""

from __future__ import annotations

import shutil
import subprocess
from dataclasses import dataclass, field
from pathlib import Path

from pypdf import PdfReader, PdfWriter

# Abaixo deste total de caracteres extraídos por pypdf, consideramos a amostra
# como "sem texto nativo" e partimos para OCR. Livros escaneados retornam ~0.
_NATIVE_TEXT_MIN_CHARS = 200


@dataclass
class ExtractionResult:
    """Texto bruto extraído + metadados de auditoria."""

    raw_text: str
    engine: str  # "native" | "ocr"
    pages: list[int]
    meta: dict = field(default_factory=dict)


def parse_page_range(spec: str) -> list[int]:
    """Converte faixas em lista 1-indexada.

    Aceita: "5", "21-30", "1-10,50-60" (vírgulas = união, ordenada, sem duplicar).
    """
    pages: list[int] = []
    for part in spec.split(","):
        part = part.strip()
        if not part:
            continue
        if "-" in part:
            start_s, end_s = part.split("-", 1)
            start, end = int(start_s), int(end_s)
            if start < 1 or end < start:
                raise ValueError(f"Faixa de páginas inválida: {part!r}")
            pages.extend(range(start, end + 1))
        else:
            page = int(part)
            if page < 1:
                raise ValueError(f"Página inválida: {part!r}")
            pages.append(page)
    # Únicos preservando ordem
    seen: set[int] = set()
    out: list[int] = []
    for p in pages:
        if p not in seen:
            seen.add(p)
            out.append(p)
    if not out:
        raise ValueError(f"Faixa de páginas vazia: {spec!r}")
    return out


def _subset_pdf(input_pdf: Path, pages: list[int], dest: Path) -> None:
    """Grava um novo PDF só com as páginas pedidas (1-indexadas)."""
    reader = PdfReader(str(input_pdf))
    total = len(reader.pages)
    writer = PdfWriter()
    for p in pages:
        if p > total:
            raise ValueError(f"Página {p} fora do total ({total}) do PDF")
        writer.add_page(reader.pages[p - 1])
    dest.parent.mkdir(parents=True, exist_ok=True)
    with dest.open("wb") as fh:
        writer.write(fh)


def _extract_native(subset_pdf: Path) -> str:
    """Extrai texto nativo do subset com pypdf, separando páginas."""
    reader = PdfReader(str(subset_pdf))
    parts: list[str] = []
    for page in reader.pages:
        parts.append(page.extract_text() or "")
    return "\n\n".join(parts)


def _run_ocr(subset_pdf: Path, languages: str, temp_dir: Path) -> str:
    """Roda OCRmyPDF no subset e devolve o texto do sidecar.

    Usamos --force-ocr porque as páginas do livro são imagens escaneadas sem
    camada de texto. --clean melhora a imagem via unpaper antes do OCR.
    """
    if shutil.which("ocrmypdf") is None:
        raise RuntimeError("ocrmypdf não encontrado no PATH")

    temp_dir.mkdir(parents=True, exist_ok=True)
    ocr_pdf = temp_dir / "ocr.pdf"
    sidecar = temp_dir / "sidecar.txt"

    cmd = [
        "ocrmypdf",
        "--force-ocr",  # páginas são imagens; força reconhecimento
        "--clean",  # unpaper limpa a imagem antes do OCR
        "--language",
        languages,
        str(subset_pdf),
        str(ocr_pdf),
        "--sidecar",
        str(sidecar),
    ]
    # capture_output para registrar falhas de OCR sem poluir o stdout do CLI.
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError(
            f"OCRmyPDF falhou (exit {proc.returncode}): {proc.stderr.strip()}"
        )
    return sidecar.read_text(encoding="utf-8", errors="replace")


def extract_pages(
    input_pdf: Path,
    pages: list[int],
    *,
    languages: str = "por+eng",
    temp_dir: Path,
) -> ExtractionResult:
    """Extrai texto da faixa de páginas, escolhendo nativo ou OCR."""
    if not input_pdf.exists():
        raise FileNotFoundError(f"PDF não encontrado: {input_pdf}")

    subset_pdf = temp_dir / "subset.pdf"
    _subset_pdf(input_pdf, pages, subset_pdf)

    native_text = _extract_native(subset_pdf)
    if len(native_text.strip()) >= _NATIVE_TEXT_MIN_CHARS:
        return ExtractionResult(
            raw_text=native_text,
            engine="native",
            pages=pages,
            meta={"native_chars": len(native_text.strip())},
        )

    # Sem texto nativo suficiente -> OCR.
    ocr_text = _run_ocr(subset_pdf, languages, temp_dir)
    return ExtractionResult(
        raw_text=ocr_text,
        engine="ocr",
        pages=pages,
        meta={
            "languages": languages,
            "native_chars": len(native_text.strip()),
            "ocr_chars": len(ocr_text.strip()),
        },
    )
