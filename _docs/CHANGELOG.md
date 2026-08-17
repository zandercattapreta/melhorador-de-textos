---
sistema: PROJETO
tipo: sessao
atualizado_em: 2026-08-16
---

# Changelog — TXTMelhorator

Histórico detalhado anterior ao reset: `_historico/2026-08-01_pre-reset/CHANGELOG.md`.

## [app] 2026-08-16 — noite (EOD+EOW; correção do P0 U1c)

- **IA residente:** GGUF carrega 1× e fica na memória (antes: 6 GiB recarregados
  por página — 17× no log); `unload_llama_model` libera ao fim da fila.
- **Sem freeze:** `propose_review`, LT local/Premium e nuvem viraram comandos
  async (fora da main thread); fila de 1 inferência por vez; Parar interrompe a
  geração no próximo token.
- **Melhorize imediato:** cada página passa pela limpeza+estrutura ao sair da
  captura (`melhorize_page`), sempre; LT/IA depois, se ligado. Passada final no
  livro inteiro continua sendo a âncora do arquivo salvo.
- **OCR normalizado** (`normalize_ocr_page_text`): barras `|` órfãs removidas,
  linha em branco espúria fundida (entrelinha larga), hífen através de branco
  junta. Causa do "texto cagado" do UAT (Schopenhauer I, pág. 12).
- **Emissão única por página** na extração (antes: vazia na passada nativa +
  real no OCR) — consertou o teste `ocr_reconhece_paginas_do_livro_real`.
  Suíte completa verde pela primeira vez.
- **UI:** página com fit na moldura; texto ao vivo acompanha a captura
  (gruda no fim, solta ao rolar); rótulo "Bruto desta página" corrigido.
- **Limpeza autorizada:** removidos caminho morto por segmentos
  (`assemble_native_page_by_segments`+helpers), bench do experimento,
  comandos órfãos (`dehyphenate_text`, `propose_heuristic_review`),
  regexes/campos mortos. Disco: debug 5,1 GiB + `_temp` intermediários (~1 GB).
- **Convenção de build:** `bash _APP/scripts/build-release.sh` →
  `_APP/version/<data_hora>/TXTMelhorator.app`. Build da noite: `2026-08-16_2232`.
- Novo QA dirigido: `page_melhorize_dump` (caminho ao vivo exato do app).

## [app] 2026-08-16 — EOD (sessão incompleta / handover)

- Rename produto **TXTMelhorator** (app, CLI `txtmelhorator`, crates, GitHub `txtmelhorator`).
- Layout B + tokens DS Melhorator; wizard de transcrição/scan.
- Revisão: aplica + Desfazer; hifenação na revisão.
- **Bloqueio:** IA local durante OCR recarrega GGUF (~6 GiB) por página → rainbow wheel; U1c marcado quebrado. Handover: `HANDOVER-2026-08-16-EOD.md`.

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
