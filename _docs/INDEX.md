---
sistema: PROJETO
tipo: indice
atualizado_em: 2026-08-16
---

# Índice — TXTMelhorator

Uma ferramenta, duas superfícies: **App desktop** (`_APP/`) e **CLI** (`_CLI/`, referência). PRD único: [`PRD-MELHORADOR.md`](PRD-MELHORADOR.md).

Retomando o trabalho? [`HANDOVER-2026-08-16.md`](HANDOVER-2026-08-16.md).

Regras: [`POLITICA_DOCS.md`](POLITICA_DOCS.md).

## Comece por aqui

| Doc | Para |
|---|---|
| [`PRD-MELHORADOR.md`](PRD-MELHORADOR.md) | **produto** — rotina do app (§5), princípios, o que não faz |
| [`HANDOVER-2026-08-16.md`](HANDOVER-2026-08-16.md) | retomar o trabalho — estado, pendências, armadilhas |
| [`BACKLOG-MELHORADOR.md`](BACKLOG-MELHORADOR.md) | **fila** R1→R5 · feito · ops |
| [`PLANO-APP-MELHORADOR.md`](PLANO-APP-MELHORADOR.md) | plano do app: stack, fases, decisões |
| [`ARQUITETURA-MELHORADOR.md`](ARQUITETURA-MELHORADOR.md) | App hoje × alvo (motores, revisão, casca) + CLI |
| [`DESIGN-SYSTEM-APP.md`](DESIGN-SYSTEM-APP.md) | Design System e telas do app |

## Arquitetura e integrações

| Doc | Conteúdo |
|---|---|
| [`arquitetura/AS_IS.md`](arquitetura/AS_IS.md) | estado em 16/Ago (App + CLI) |
| [`arquitetura/ARQUITETURA.md`](arquitetura/ARQUITETURA.md) | atalho → ARQUITETURA-MELHORADOR |
| [`integracoes/LANGUAGETOOL.md`](integracoes/LANGUAGETOOL.md) | revisão manual |
| [`integracoes/SADE.md`](integracoes/SADE.md) | visão de worker |

## Operação

[`operacao/CLI.md`](operacao/CLI.md) · [`MELHORADOR_GUIA_RAPIDO.md`](MELHORADOR_GUIA_RAPIDO.md) (CLI, lote)

## Governança

[`POLITICA_DOCS.md`](POLITICA_DOCS.md) · [`GLOSSARIO.md`](GLOSSARIO.md) · [`CHANGELOG.md`](CHANGELOG.md) · [`ROADMAP.md`](ROADMAP.md)

## Histórico

- `_historico/2026-08-16_PRD-pre-pivo.md` — PRD curto de 01/Ago (só CLI)
- `_historico/2026-08-16_BACKLOG-pre-rotina.md` — backlog P0–P2 de 15/Ago
- `_historico/2026-08-01_pre-reset/` — backlog/changelog/QA antigos + `docs/INTEGRACAO_SADE.md` e `ZBOOKER_FERRAMENTAS.md`

## Validar

```bash
bash "../~scripts/docs/check-docs.sh" .
```
