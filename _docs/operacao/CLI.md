---
sistema: MELHORADOR
tipo: operacao
atualizado_em: 2026-08-01
---

# Operação — CLI

## Deps nativas (macOS)

```bash
brew install python@3.12 tesseract tesseract-lang ghostscript qpdf unpaper
```

## Ambiente

```bash
python3.12 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
python -m pytest   # 35 testes esperados
```

## Comandos

```bash
melhorador-textos extract \
  --input "_ originais/<arquivo>.pdf" \
  --pages 21-30 \
  --name mesopotamia

melhorador-textos prepare-lt \
  --input "_output/mesopotamia/pages-021-030/cleaned.md"

melhorador-textos import-lt \
  --original  "_output/.../languagetool/original.txt" \
  --corrected "_output/.../languagetool/corrected.md"
```

OCR de livro inteiro (centenas de páginas) = ação APAE — não rodar sem autorização.
