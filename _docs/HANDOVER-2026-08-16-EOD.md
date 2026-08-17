---
sistema: MELHORADOR
tipo: sessao
atualizado_em: 2026-08-16
---

# HANDOVER — 16/Ago/2026 (EOD + EOW)

Zander encerra sessão: revisão ao vivo com IA **não está funcionando**; passa para outra IA.

## Estado atual

- Repo: `github.com/zandercattapreta/txtmelhorator`
- Pasta: `_ TXTMelhorator`
- Branch: `feature/backlog-r1-r5-close`
- Produto renomeado: **TXTMelhorator** · DS: **DS Melhorator** (Z Tinta rejeitado)
- Layout B no app (barra única + 2 colunas)
- Wizard de config (transcrição/scan/OCR — não “livro”)
- Revisão: aplicar + Desfazer (não checklist)
- IA in-process (`llama-cpp-2` + Metal) existe, mas…

## P0 — Bloqueador (próxima IA)

**Revisão com Gemma durante o OCR trava o Mac (rainbow wheel) e o texto na caixa não fica como pedido.**

Evidência (log sessão `316539`):

- **17×** `print_info: file size = 6.23 GiB` = modelo **recarregado 17 vezes**
- `llama_infer::generate` carrega GGUF, gera, libera — **por chamada**
- `propose_review` é comando Tauri **síncrono** (sem `async`)
- UI chama isso **por página** em paralelo com OCR

Pedido do Zander (não negociar embora): captura **e** revisão **ao mesmo tempo**, texto **já melhorado** na caixa, sem travar. Ele **rejeitou** “IA só no fim” como substituto do pedido.

## O que NÃO fazer

- Não “só propõe / checklist Aplicar” de novo
- Não voltar a Z Tinta
- Não sair rodando `tauri dev` / bench GGUF sem autorização (já congelou o app várias vezes)
- Não apagar goldens em `_temp/goldens/`

## Feito nesta sessão (parcial)

- Rename TXTMelhorator (código + GitHub + pasta)
- Layout B + tokens DS
- Wizard; wording “transcrição/scan”
- Apply + Desfazer; hifenação na revisão
- Tentativa de revisão ao vivo (quebrada — ver P0)

## Prompt para o próximo agente

1. Ler este handover + `AGENTS.md` + `.agent/brain/EXPERIMENT_REVIEW_LIVE.md` (se existir).
2. **Diagnosticar/pensar** antes de executar; Zander pediu PARE quando saímos correndo.
3. Resolver P0: IA durante captura **sem** reload 6 GB/página e **sem** bloquear UI — mantendo o pedido de paralelismo.
4. UAT com Zander antes de declarar pronto.

## P1 / P2 / P3

| Pri | Item |
|---|---|
| P0 | IA/LT ao vivo sem freeze; texto revisado visível de verdade |
| P1 | U8–U10 polish DS / fila / diff viewer |
| P2 | Builds Linux/Windows + Releases |
| P3 | hOCR tabelas fino |

## DoD desta sessão

**Não atendido** no critério de uso: app trava; melhoria em tempo real falhou.
