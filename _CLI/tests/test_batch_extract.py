# ==============================================================================
# SCRIPT: test_batch_extract.py
# DESCRIÇÃO: Testes unitários do orquestrador batch (sem OCR real)
# CHAMADO POR: pytest (python -m pytest)
# DEPENDÊNCIAS: pytest, txtmelhorator.batch_extract
# CONTRATO (RESPOSTA ESPERADA): todos os testes passam (exit 0)
# ==============================================================================
"""Testes de descoberta de PDFs, resolução de faixas e checkpoint."""

from __future__ import annotations

import json

import pytest

from txtmelhorator.batch_extract import (
    BatchReport,
    BookRecord,
    DEFAULT_SAMPLE_PAGES,
    discover_pdfs,
    load_completed_slugs,
    resolve_pages,
    save_checkpoint,
)


# --- discover_pdfs ------------------------------------------------------------

def test_discover_pdfs_ordena_e_filtra(tmp_path):
    (tmp_path / "b.pdf").write_bytes(b"")
    (tmp_path / "a.pdf").write_bytes(b"")
    (tmp_path / "nota.txt").write_text("não sou pdf")
    pdfs = discover_pdfs(tmp_path)
    assert [p.name for p in pdfs] == ["a.pdf", "b.pdf"]


def test_discover_pdfs_dir_inexistente(tmp_path):
    with pytest.raises(FileNotFoundError):
        discover_pdfs(tmp_path / "nao-existe")


# --- resolve_pages ------------------------------------------------------------

def test_resolve_pages_padrao_amostra():
    # Sem spec → amostra padrão 1-50.
    assert DEFAULT_SAMPLE_PAGES == "1-50"
    assert resolve_pages(None, 578) == list(range(1, 51))


def test_resolve_pages_recorta_ao_total():
    # Livro menor que a amostra: recorta em vez de estourar o PDF.
    assert resolve_pages(None, 30) == list(range(1, 31))


def test_resolve_pages_full_ignora_amostra():
    assert resolve_pages(None, 12, full=True) == list(range(1, 13))


def test_resolve_pages_spec_explicita():
    assert resolve_pages("21-30", 578) == list(range(21, 31))


def test_resolve_pages_alem_do_total():
    with pytest.raises(ValueError):
        resolve_pages("100-110", 50)


# --- checkpoint ---------------------------------------------------------------

def _report_com_livros() -> BatchReport:
    report = BatchReport(
        batch_id="t", timestamp="t", input_dir="_originais", output_dir="_output"
    )
    report.books = [
        BookRecord(
            name="ok", slug="ok", input_pdf="a.pdf", pages=(1, 50), status="success"
        ),
        BookRecord(
            name="ruim", slug="ruim", input_pdf="b.pdf", pages=(1, 50), status="failed"
        ),
    ]
    return report


def test_checkpoint_roundtrip_json(tmp_path):
    # Grava e relê: só slugs com sucesso contam como concluídos.
    path = tmp_path / "checkpoint.json"
    save_checkpoint(path, _report_com_livros())
    assert json.loads(path.read_text(encoding="utf-8"))["books"]
    assert load_completed_slugs(path) == {"ok"}


def test_checkpoint_ausente_ou_corrompido(tmp_path):
    assert load_completed_slugs(tmp_path / "nada.json") == set()
    corrompido = tmp_path / "ruim.json"
    corrompido.write_text("{não é json", encoding="utf-8")
    assert load_completed_slugs(corrompido) == set()
