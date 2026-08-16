---
sistema: PROJETO
tipo: sessao
atualizado_em: 2026-08-16
---

# Changelog — Melhorador de Textos

Histórico detalhado anterior ao reset: `_historico/2026-08-01_pre-reset/CHANGELOG.md`.

## [docs] 2026-08-16

- PRD único: `PRD-MELHORADOR.md` cobre App desktop + CLI. `PRD.md` rotacionado para `_historico/2026-08-16_PRD-pre-pivo.md`.
- `INDEX.md` aponta só esse PRD.
- AS_IS, AGENTS, README, plano do app e arquitetura alinhados ao estado real (App existe; CLI é referência).
- Qualidade app (modo aprimorado): sumário multilinha não vira `##`; linhas nativas de uma coluna ordenadas de cima para baixo; transporte de fragmento na virada de página (2–3 letras). Goldens intactos.
- Rotina-alvo do app no PRD §5 (PDF/pasta → idioma → nativo/OCR → melhorar M1–M10 → salvar no fim). Emenda IA: só revisão opt-in + vocabulário da fonte. Backlog R1–R5.
- `ARQUITETURA-MELHORADOR.md`: mapa App hoje × alvo (motores, revisão, casca/SO) alinhado ao PRD.
- Backlog reescrito em R1→R5; `BACKLOG.md` rotacionado para `_historico/2026-08-16_BACKLOG-pre-rotina.md`.

## [docs] 2026-08-01

- Reset de documentação (AS IS, política, PRD, backlog, operação CLI).
- PoC de código inalterada (0.1.0); 35 testes unitários verdes na verificação do reset.
