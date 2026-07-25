# ==============================================================================
# SCRIPT: test_languagetool_review.py
# DESCRIÇÃO: Testes do fluxo manual LanguageTool (manifesto, hash, diff)
# CHAMADO POR: pytest
# DEPENDÊNCIAS: pytest, melhorador_textos.languagetool_review
# CONTRATO (RESPOSTA ESPERADA): asserts de manifesto, chunking e diff
# ==============================================================================

import json

from melhorador_textos.languagetool_review import (
    chunk_text,
    import_correction,
    prepare_package,
    sha256_text,
)


def test_prepare_package_writes_manifest(tmp_path):
    text = "Parágrafo um.\n\nParágrafo dois."
    pkg = prepare_package(text, tmp_path)
    assert pkg.original_path.exists()
    assert pkg.manifest_path.exists()
    manifest = json.loads(pkg.manifest_path.read_text(encoding="utf-8"))
    assert manifest["language"] == "pt-BR"
    assert manifest["source_sha256"] == sha256_text(text)
    assert manifest["char_count"] == len(text)


def test_chunking_respects_limit():
    # Um parágrafo gigante deve ser fatiado abaixo do limite.
    big = "a" * 120_000
    chunks = chunk_text(big, size=50_000)
    assert len(chunks) == 3
    assert all(len(c) <= 50_000 for c in chunks)


def test_chunking_keeps_paragraphs_together():
    text = "\n\n".join(["p1", "p2", "p3"])
    chunks = chunk_text(text, size=50_000)
    assert len(chunks) == 1
    assert "p1" in chunks[0] and "p3" in chunks[0]


def test_import_correction_generates_diff(tmp_path):
    original = tmp_path / "original.txt"
    corrected = tmp_path / "corrected.md"
    original.write_text("texto com erro", encoding="utf-8")
    corrected.write_text("texto sem erro", encoding="utf-8")
    # Prepara manifesto para validar o hash.
    prepare_package("texto com erro", tmp_path)
    result = import_correction(
        original, corrected, tmp_path, manifest_path=tmp_path / "manifest.json"
    )
    assert result.changed is True
    assert result.diff_path.exists()
    assert "erro" in result.diff
    assert result.source_hash_matches is True


def test_import_correction_detects_hash_mismatch(tmp_path):
    original = tmp_path / "original.txt"
    corrected = tmp_path / "corrected.md"
    prepare_package("texto base correto", tmp_path)
    # Original adulterado após o prepare -> hash não confere.
    original.write_text("texto base adulterado", encoding="utf-8")
    corrected.write_text("texto base adulterado revisado", encoding="utf-8")
    result = import_correction(
        original, corrected, tmp_path, manifest_path=tmp_path / "manifest.json"
    )
    assert result.source_hash_matches is False


def test_no_changes_produces_empty_diff(tmp_path):
    original = tmp_path / "original.txt"
    corrected = tmp_path / "corrected.md"
    original.write_text("igual", encoding="utf-8")
    corrected.write_text("igual", encoding="utf-8")
    result = import_correction(original, corrected, tmp_path)
    assert result.changed is False
    assert result.diff == ""
