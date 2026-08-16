---
sistema: MELHORADOR
tipo: as-is
atualizado_em: 2026-08-16
---

# AS IS — TXTMelhorator

**Levantado em:** 16/Ago/2026
**Método:** código em `_CLI/` e `_APP/`, testes, `BATCH_REPORT.json`, HANDOVER, PRD único.

> Retrato do que existe. Não é alvo nem plano.

---

## 1. O que é

Uma ferramenta, duas superfícies. Extrai texto de PDF (nativo ou OCR), limpa e estrutura em Markdown. **Sem IA no pipeline.**

| Item | Valor |
|---|---|
| Produto | PoC **0.2.0** |
| App | Tauri 2 + React 19 + core Rust (`_APP/`) — drop PDF, OCR ao vivo, exporta `.melhorado.md` |
| CLI | Python 3.12 (`_CLI/`) — referência + lote |
| Testes | **61** pytest · **33** cargo `--release` (goldens dos 4 livros) |
| Git | `main` em GitHub; working tree sujo (`_APP/` untracked, CLI movido) |
| Lote CLI | 4/4 livros (3,5 mil págs.); Paideia: LT local timeout |

## 2. Pipeline

**App:** PDF → PDFium (nativo) ou Tesseract (OCR) → `clean_text_enhanced` → `apply_structure_enhanced` → preview → `.melhorado.md` ao lado da origem.

**CLI:** `_originais/*.pdf` → `batch-extract` → `raw.txt` + `cleaned.md` + `report.json` → `check-lt` / `prepare-lt`.

Dois modos no core Rust: **paridade** (= CLI, goldens) e **aprimorado** (app). Divergências intencionais, comentadas no código.

## 3. Código

| Peça | Papel |
|---|---|
| `_CLI/src/txtmelhorator/` | extração, limpeza, estrutura, metadados, batch, LT |
| `_APP/core/` | port Rust + extração PDFium/Tesseract + montador nativo v3 |
| `_APP/src-tauri/` | comandos Tauri (`process_pdf`, `process_text_file`, `save_result`) |
| `_APP/src/App.tsx` | dropzone, progresso, preview, salvar |

## 4. Acervo

Processados no lote: Schopenhauer I–II, Pierre Levêque, Paideia. Na pasta e ainda fora do lote: Verne, D&D, Pinocchio (2 eds.).

Metadados CLI: ficha quando o PDF tem texto nativo; senão cai no nome do arquivo (Pierre).

## 5. Zero-IA

Pipeline = Tesseract clássico + regras. LanguageTool = revisão (CLI local hoje; app ainda não). IA local = fase E5, não está no produto.

## 6. Dívida visível

| # | Item |
|---|---|
| 1 | `_docs/` fora do git |
| 2 | `_APP/` não commitado |
| 3 | Tesseract do app vem do Homebrew (não dá para distribuir o `.app` sozinho) |
| 4 | Sumário multilinha, tabelas, notas de rodapé, ordem de linhas nativas (v3) |
| 5 | Sem fila, pasta, LT, docx no app |
| 6 | `plugin-dialog` no npm, não ligado no Rust |
