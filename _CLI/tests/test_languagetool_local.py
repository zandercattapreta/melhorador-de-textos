# ==============================================================================
# SCRIPT: test_languagetool_local.py
# DESCRIÇÃO: Testes unitários da revisão LT local (lógica pura, sem servidor)
# CHAMADO POR: pytest (python -m pytest)
# DEPENDÊNCIAS: pytest, melhorador_textos.languagetool_local
# CONTRATO (RESPOSTA ESPERADA): todos os testes passam (exit 0)
# ==============================================================================
"""Testes de apply_suggestions e reajuste de offsets por chunk."""

from __future__ import annotations

from melhorador_textos.languagetool_local import apply_suggestions, check_text


def _match(offset: int, length: int, *replacements: str) -> dict:
    return {
        "offset": offset,
        "length": length,
        "replacements": [{"value": r} for r in replacements],
    }


# --- apply_suggestions --------------------------------------------------------

def test_aplica_primeira_sugestao():
    text = "Ele foi na escola."
    #       0123456789
    corrected, applied = apply_suggestions(text, [_match(4, 6, "foi à")])
    assert corrected == "Ele foi à escola."
    assert applied == 1


def test_ignora_match_sem_sugestao():
    text = "Palavra ilegível aqui."
    corrected, applied = apply_suggestions(
        text, [{"offset": 8, "length": 8, "replacements": []}]
    )
    assert corrected == text
    assert applied == 0


def test_ignora_sobreposicao():
    text = "abcdef"
    matches = [_match(0, 3, "XYZ"), _match(2, 2, "!!")]  # 2º sobrepõe o 1º
    corrected, applied = apply_suggestions(text, matches)
    assert corrected == "XYZdef"
    assert applied == 1


def test_multiplas_correcoes_em_ordem():
    text = "aa bb cc"
    matches = [_match(6, 2, "CC"), _match(0, 2, "AA")]  # fora de ordem
    corrected, applied = apply_suggestions(text, matches)
    assert corrected == "AA bb CC"
    assert applied == 2


def test_match_fora_do_texto_e_ignorado():
    text = "curto"
    corrected, applied = apply_suggestions(text, [_match(3, 10, "x")])
    assert corrected == text
    assert applied == 0


def test_sem_matches_texto_intacto():
    text = "Nada a corrigir.\n\nSegundo parágrafo."
    corrected, applied = apply_suggestions(text, [])
    assert corrected == text
    assert applied == 0


# --- check_text: offsets por chunk --------------------------------------------

def test_check_text_reajusta_offsets_por_chunk(monkeypatch):
    # Dois parágrafos que forçam 2 chunks (limite pequeno via monkeypatch).
    para1 = "primeiro paragrafo"
    para2 = "segundo paragrafo"
    text = f"{para1}\n\n{para2}"

    import melhorador_textos.languagetool_local as ltl

    # Cada chunk = 1 parágrafo (chunk_text com size pequeno).
    monkeypatch.setattr(
        ltl, "chunk_text", lambda t, **kw: t.split("\n\n")
    )
    # Servidor falso: aponta a 1ª palavra de cada chunk (offset local 0).
    monkeypatch.setattr(
        ltl,
        "_check_chunk",
        lambda chunk, url: [
            {"offset": 0, "length": len(chunk.split()[0]), "replacements": []}
        ],
    )

    matches = ltl.check_text(text)
    offsets = [m["offset"] for m in matches]
    # 1º chunk começa em 0; 2º chunk começa depois de para1 + "\n\n".
    assert offsets == [0, len(para1) + 2]
