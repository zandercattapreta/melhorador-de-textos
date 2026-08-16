---
sistema: MELHORADOR
tipo: prd
atualizado_em: 2026-08-01
---

# PRD — Melhorador de Textos (arquivo · 01/Ago/2026)

Rotacionado em 16/Ago/2026. O PRD vigente é [`../PRD-MELHORADOR.md`](../PRD-MELHORADOR.md).

---

Extrair e tornar legível o texto de PDFs de livros digitalizados, com fidelidade à fonte e sem IA generativa.

## 1. Visão

Livros escaneados chegam ilegíveis (OCR sujo, quebras, lixo de página). O Melhorador produz Markdown limpo e auditável para revisão humana / ingestão editorial — não um “autor automático”.

## 2. Princípios

| Princípio | Prática |
|---|---|
| **Fidelidade** | não inventar nem completar conteúdo ausente |
| **Zero-IA** | OCR clássico + heurísticas + LT humano |
| **Auditável** | hashes e `report.json` em toda faixa |
| **Local-only para obras** | PDF bruto fora do git |

## 3. Requisitos

| ID | Requisito | Estado |
|---|---|---|
| R1 | Extrair faixa de páginas (nativo/OCR) | atendido |
| R2 | Limpeza determinística + Markdown estruturado | atendido |
| R3 | Relatório com métricas/hashes | atendido |
| R4 | Fluxo LanguageTool auditável | atendido (manual) |
| R5 | Testes unitários da limpeza/estrutura | atendido (35) |
| R6 | Worker/API SADE | **não** |
| R7 | OCR do livro completo Mesopotâmia | **não** (APAE) |
| R8 | Docs versionadas no git | **não** (`_docs/` ignorado) |

## 4. Fora de escopo

- Reescrita literária / estilo.
- Cloud OCR pago / LLM.
- EPUB final (ZBOOKER).
