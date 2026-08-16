# ==============================================================================
# SCRIPT: test_structure.py
# DESCRIÇÃO: Testes de detecção H1–H4 e SUMÁRIO (sem IA)
# CHAMADO POR: pytest
# DEPENDÊNCIAS: pytest, txtmelhorator.structure
# CONTRATO (RESPOSTA ESPERADA): asserts de headings vs prosa
# ==============================================================================

from txtmelhorator.structure import apply_structure


def test_short_numbered_becomes_h2():
    text = "3. Os avanços de uma racionalidade nova\n\nProsa a seguir."
    result = apply_structure(text)
    assert "## 3. Os avanços de uma racionalidade nova" in result.text
    assert result.stats["h2"] == 1
    assert "Prosa a seguir." in result.text


def test_long_numbered_stays_prose():
    text = (
        "1. O rei exerce uma autoridade absoluta sobre a comunidade que se "
        "incarna nele. Mas o seu poder autocrático não é um constrangimento."
    )
    result = apply_structure(text)
    assert not result.text.startswith("#")
    assert result.stats["prose"] == 1
    assert result.stats["h2"] == 0


def test_h3_and_h4_from_dotted_numbers():
    text = "1.2 Subseção curta\n\n1.2.3 Detalhe curto\n\nCorpo."
    result = apply_structure(text)
    assert "### 1.2 Subseção curta" in result.text
    assert "#### 1.2.3 Detalhe curto" in result.text
    assert result.stats["h3"] == 1
    assert result.stats["h4"] == 1


def test_all_caps_becomes_h1():
    text = "AS PRIMEIRAS CIVILIZAÇÕES\n\nTexto do capítulo."
    result = apply_structure(text)
    assert "# AS PRIMEIRAS CIVILIZAÇÕES" in result.text
    assert result.stats["h1"] == 1


def test_colophon_not_h1():
    text = "DEPÓSITO LEGAL Nº 299162/09\n\nCorpo do livro."
    result = apply_structure(text)
    assert not result.text.startswith("#")
    assert result.stats["h1"] == 0


def test_named_preambulo_is_h1():
    text = "Preâmbulo\n\nComeça o texto."
    result = apply_structure(text)
    assert "# Preâmbulo" in result.text


def test_sumario_block():
    text = (
        "SUMÁRIO\n\n"
        "Capítulo I .......... 12\n\n"
        "Capítulo II ......... 45\n\n"
        "1. Introdução longa demais para ser entrada de sumário e que deve "
        "encerrar o modo TOC porque passa do limite e não tem pontilhado."
    )
    result = apply_structure(text)
    assert "# SUMÁRIO" in result.text
    assert "- Capítulo I .......... 12" in result.text
    assert "- Capítulo II ......... 45" in result.text
    assert result.stats["toc_entries"] == 2
    assert result.stats["h1"] == 1


def test_does_not_invent_heading_from_normal_prose():
    text = "Pode entretanto fazer-se uma reflexão sobre o poder."
    result = apply_structure(text)
    assert result.text.strip() == text
    assert result.stats["prose"] == 1


def test_title_case_short_between_prose_becomes_h2():
    previous = "Parágrafo anterior longo. " * 10
    following = "Parágrafo seguinte longo. " * 10
    text = f"{previous}\n\nO tempo absoluto\n\n{following}"
    result = apply_structure(text)
    assert "## O tempo absoluto" in result.text
    assert result.stats["title_case_headings"] == 1


def test_short_sentence_with_period_does_not_become_heading():
    previous = "Parágrafo anterior longo. " * 10
    following = "Parágrafo seguinte longo. " * 10
    text = f"{previous}\n\nEsta é uma frase curta.\n\n{following}"
    result = apply_structure(text)
    assert "## Esta é uma frase curta." not in result.text


def test_corrects_roman_ii_ocr_only_in_heading():
    text = "IL - OS ANTROPIANOS ATÉ À METALURGIA\n\n" + ("Prosa longa. " * 20)
    result = apply_structure(text)
    assert "# II. — OS ANTROPIANOS ATÉ À METALURGIA" in result.text
    assert result.stats["heading_ocr_corrections"] == 1
