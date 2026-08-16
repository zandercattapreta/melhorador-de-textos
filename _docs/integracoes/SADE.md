---
sistema: MELHORADOR
tipo: integracao
atualizado_em: 2026-08-01
---

# SADE (visão)

**Estado:** sem código de integração. O Melhorador é CLI local.

## Papel pretendido

| Hoje (Melhorador) | Futuro (SADE) |
|---|---|
| PDF em `_ originais/` | asset editorial / upload |
| `_output/` Markdown + report | ingestão em pacote de revisão |
| Job lançado no terminal | worker/job no hub |

Contrato natural de I/O: PDF + faixa de páginas → `cleaned.md` + `report.json` (+ diff LT opcional). Manter **zero-IA** no worker.

## ZBOOKER

Índices/citações/EPUB são do ecossistema editorial SADE/ZBOOKER — não deste CLI. Ponteiro histórico: `_historico/2026-08-01_pre-reset/docs/ZBOOKER_FERRAMENTAS.md` (SSOT no SADE).
