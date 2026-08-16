# ==============================================================================
# SCRIPT: test_ocr_typos.py / test_page_range
# DESCRIÇÃO: Typos OCR + faixas não contíguas
# ==============================================================================

from txtmelhorator.extraction import parse_page_range
from txtmelhorator.ocr_typos import apply_ocr_typos


def test_parse_page_range_comma():
    assert parse_page_range("1-3,10,50-51") == [1, 2, 3, 10, 50, 51]


def test_ocr_typos_fi_ligature():
    text, n = apply_ocr_typos("aﬁrmar")
    assert text == "afirmar"
    assert n == 1
