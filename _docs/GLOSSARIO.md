---
sistema: PROJETO
tipo: glossario
atualizado_em: 2026-08-01
---

# Glossário

| Termo | Significa | Prova |
|---|---|---|
| **TXTMelhorator** | este CLI / pacote `txtmelhorator` | `pyproject.toml` |
| **extract** | comando: PDF → raw + cleaned + report | `cli.py` |
| **engine** | `native` ou `ocr` na extração | `report.json` → `engine` |
| **cleaned.md** | texto limpo + estrutura Markdown | `_output/.../cleaned.md` |
| **structure** | H1–H4 / SUMÁRIO por heurística | `structure.py` |
| **prepare-lt / import-lt** | ida e volta LanguageTool manual | `languagetool_review.py` |
| **zero-IA** | proibição de LLM/OCR generativo no pipeline | `AGENTS.md` |

## Termos proibidos

| Não usar | Porque | Usar |
|---|---|---|
| "usa GPT/Claude para limpar" | proibido no projeto | heurísticas + OCR clássico |
| "API LanguageTool ativa" | PoC é revisão manual | fluxo prepare-lt / import-lt |
| "integrado ao SADE" | só visão em doc | CLI local; integração = alvo |
| "stack a definir" | PoC 0.1.0 existe | Python 3.12 + OCRmyPDF |
