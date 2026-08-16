# ==============================================================================
# SCRIPT: ocr_typos.py
# DESCRIÇÃO: Dicionário determinístico de typos OCR (paridade com core Rust)
# CHAMADO POR: cleanup (opcional) / testes
# CONTRATO (RESPOSTA ESPERADA): (texto, n_substituições); zero invenção
# ==============================================================================

from __future__ import annotations

OCR_TYPOS: tuple[tuple[str, str], ...] = (
    (" rn ", " m "),
    (" cl ", " d "),
    (" vv ", " w "),
    ("ﬁ", "fi"),
    ("ﬂ", "fl"),
    ("—-", "—"),
)


def apply_ocr_typos(text: str) -> tuple[str, int]:
    """Aplica pares fixos; conta substituições."""
    out = text
    n = 0
    for bad, good in OCR_TYPOS:
        before = out.count(bad)
        if before:
            out = out.replace(bad, good)
            n += before
    return out, n
