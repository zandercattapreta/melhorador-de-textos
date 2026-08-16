# ==============================================================================
# SCRIPT: test_cleanup.py
# DESCRIÇÃO: Testes da limpeza determinística (sem inventar conteúdo)
# CHAMADO POR: pytest
# DEPENDÊNCIAS: pytest, melhorador_textos.cleanup
# CONTRATO (RESPOSTA ESPERADA): asserts de normalização e preservação
# ==============================================================================

from melhorador_textos.cleanup import clean_text


def test_remove_invisible_chars():
    # zero-width space e BOM devem sumir.
    raw = "pala\u200bvra\ufeff final"
    result = clean_text(raw, reflow=False)
    assert "\u200b" not in result.text
    assert "\ufeff" not in result.text
    assert "palavra final" in result.text
    # ftfy pode remover parte dos invisíveis antes da nossa contagem;
    # o essencial é que nenhum sobre no texto final.
    assert result.stats["invisible_removed"] >= 1


def test_dehyphenate_line_break():
    # "conheci-\nmento" -> "conhecimento"
    raw = "o conheci-\nmento humano"
    result = clean_text(raw, reflow=False)
    assert "conhecimento" in result.text
    assert result.stats["hyphenations_joined"] == 1


def test_dehyphenate_preserves_legitimate_hyphen():
    # Hífen composto no meio da linha não deve ser tocado.
    raw = "bem-estar social"
    result = clean_text(raw, reflow=False)
    assert "bem-estar" in result.text


def test_remove_page_markers():
    raw = "linha um\n-- 3 of 578 --\nlinha dois"
    result = clean_text(raw, reflow=False)
    assert "of 578" not in result.text
    assert result.stats["page_markers_removed"] == 1


def test_collapse_whitespace():
    raw = "muito    espaço\n\n\n\nfim"
    result = clean_text(raw, reflow=False)
    assert "muito espaço" in result.text
    # 3+ quebras viram no máximo 2.
    assert "\n\n\n" not in result.text


def test_reflow_joins_paragraph_lines():
    raw = "primeira linha\nsegunda linha\n\nnovo parágrafo"
    result = clean_text(raw, reflow=True)
    assert "primeira linha segunda linha" in result.text
    assert "novo parágrafo" in result.text


def test_does_not_invent_content():
    # A limpeza não deve adicionar palavras novas.
    raw = "texto simples de teste"
    result = clean_text(raw, reflow=True)
    for word in ["texto", "simples", "de", "teste"]:
        assert word in result.text
    # Nenhuma palavra inventada além das originais.
    assert set(result.text.split()) == {"texto", "simples", "de", "teste"}


def test_flags_replacement_char():
    raw = "algo ilegível \ufffd aqui"
    result = clean_text(raw, reflow=False)
    assert result.stats["replacement_chars"] == 1
    assert result.warnings


def test_strips_running_headers_and_page_numbers():
    # Sidecar OCRmyPDF: páginas separadas por form-feed, cabeçalho no topo,
    # número no rodapé. Cabeçalhos alternados precisam ocorrer >= 2 vezes.
    raw = (
        "As PRIMEIRAS CIVILIZAÇÕES\n"
        "primeiro parágrafo da página um\n"
        "continua aqui\n"
        "22\n"
        "\f"
        "PREAMBULO\n"
        "segundo parágrafo da página dois\n"
        "23\n"
        "\f"
        "As PRIMEIRAS CIVILIZAÇÕES\n"
        "terceiro parágrafo da página três\n"
        "24\n"
        "\f"
        "PREAMBULO\n"
        "quarto parágrafo da página quatro\n"
        "25\n"
    )
    result = clean_text(raw, reflow=True)
    assert "As PRIMEIRAS CIVILIZAÇÕES" not in result.text
    assert "PREAMBULO" not in result.text
    assert "22" not in result.text
    assert "23" not in result.text
    assert "24" not in result.text
    assert "primeiro parágrafo" in result.text
    assert "segundo parágrafo" in result.text
    assert "terceiro parágrafo" in result.text
    assert "quarto parágrafo" in result.text
    assert result.stats["headers_removed"] >= 4
    assert result.stats["page_numbers_removed"] >= 4


def test_strips_page_number_glued_to_header():
    raw = (
        "As PRIMEIRAS CIVILIZAÇÕES\n"
        "corpo um\n"
        "22\n"
        "\f"
        "As PRIMEIRAS CIVILIZAÇÕES\n"
        "corpo dois\n"
        "23\n"
        "\f"
        "23 As PRIMEIRAS CIVILIZAÇÕES\n"
        "corpo três\n"
        "24\n"
    )
    result = clean_text(raw, reflow=True)
    assert "As PRIMEIRAS CIVILIZAÇÕES" not in result.text
    assert "corpo um" in result.text
    assert "corpo três" in result.text


def test_strips_edge_garbage_and_accent_variants():
    # PREAMBULO / PREÂMBULO devem ser o mesmo cabeçalho; aspas órfãs somem.
    raw = (
        "PREAMBULO\n"
        "corpo um\n"
        '"\n'
        "\f"
        "PREÂMBULO\n"
        "corpo dois\n"
        "25\n"
        "\f"
        "PREAMBULO\n"
        "corpo três\n"
        "26\n"
    )
    result = clean_text(raw, reflow=True)
    assert "PREAMBULO" not in result.text
    assert "PREÂMBULO" not in result.text
    assert "corpo um" in result.text
    assert "corpo dois" in result.text
    assert result.stats["edge_garbage_removed"] >= 1

def test_standalone_page_numbers_without_formfeed():
    raw = "linha útil\n42\noutra linha"
    result = clean_text(raw, reflow=False)
    assert "42" not in result.text
    assert "linha útil" in result.text
    assert result.stats["page_numbers_removed"] == 1


def test_drops_leading_front_matter_pages():
    raw = (
        "CAPA\ncréditos\n1\n"
        "\f"
        "FICHA\nISBN 123\n2\n"
        "\f"
        "CAPÍTULO\ntexto do corpo\n3\n"
    )
    result = clean_text(raw, drop_leading_pages=2)
    assert "CAPA" not in result.text
    assert "FICHA" not in result.text
    assert "texto do corpo" in result.text
    assert result.stats["leading_pages_dropped"] == 2


def test_deduplicates_scanned_pages_and_keeps_cleaner_copy():
    raw = (
        "CABECALHO\ntexto correcto da página\n14\n"
        "\f"
        "CABECALHO\nt|exto10Corrompido da página\n14\n"
        "\f"
        "CABECALHO\npágina seguinte\n15\n"
    )
    result = clean_text(raw)
    assert "texto correcto" in result.text
    assert "Corrompido" not in result.text
    assert result.stats["duplicate_pages_removed"] == 1


def test_removes_inline_page_number_after_hyphen_without_inventing_letters():
    raw = "período imen-\n10 de todos os grupos"
    result = clean_text(raw, reflow=False)
    assert "10 de todos" not in result.text
    assert "imende todos" in result.text
    assert result.stats["inline_page_numbers_removed"] == 1


def test_splits_numbered_section_glued_to_previous_paragraph():
    raw = "Fim da análise. 4. Nova seção começa aqui"
    result = clean_text(raw, reflow=False)
    assert "Fim da análise.\n\n4. Nova seção" in result.text
    assert result.stats["embedded_sections_split"] == 1


def test_removes_isolated_vertical_bars_and_garbage_line():
    raw = "Texto | com barra\n\n—<——S mo dá\n\nTexto válido"
    result = clean_text(raw, reflow=False)
    assert "|" not in result.text
    assert "—<" not in result.text
    assert "Texto válido" in result.text


def test_removes_short_edge_fragment_before_running_header():
    # Cabeçalho corrente exige >= 2 ocorrências; 3 páginas dão margem.
    raw = (
        "CABECALHO RECORRENTE\ncorpo da página um\n1\n"
        "\f"
        "dá\nCABECALHO RECORRENTE\ncorpo da página dois\n2\n"
        "\f"
        "CABECALHO RECORRENTE\ncorpo da página três\n3\n"
    )
    result = clean_text(raw)
    assert "dá" not in result.text
    assert "CABECALHO RECORRENTE" not in result.text
    assert "corpo da página dois" in result.text
    assert "corpo da página três" in result.text
