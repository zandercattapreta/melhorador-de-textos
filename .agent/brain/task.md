# Task — 16/Ago/2026 (retomada pós-EOD)

Modo: Desenvolvimento (APAE ativo). Branch `feature/backlog-r1-r5-close` (limpa, sincronizada).

## P0 — único objetivo até UAT passar

**U1c: revisão (LT/IA) em paralelo com o OCR, sem travar, com texto já melhorado na caixa.**

Causa raiz (evidência no log 316539): `llama_infer::generate` carrega e libera o GGUF de 6,23 GiB **a cada chamada** (17 recargas numa sessão) e `propose_review` é comando Tauri **síncrono** — a UI chama por página durante o OCR → rainbow wheel.

Feito 16/Ago (aguardando UAT do Zander):
1. ✅ Modelo residente (carrega 1×; recarrega só na troca de GGUF; `unload_llama_model` ao fim da fila). Recarga por página = erro crasso, NUNCA reintroduzir.
2. ✅ `propose_review`/LT/nuvem async (fora da main thread); fila = Mutex do LlamaState; Parar interrompe a geração.
3. ✅ Melhorize IMEDIATO por página na caixa ao vivo (`melhorize_page`, sem IA), sempre — LT/IA depois, se ligado. Passada final no livro inteiro segue sendo a âncora do salvo.
4. ✅ Limpeza autorizada: removidos `assemble_native_page_by_segments`+`Frag`+`frags_to_lines`, bench `bench_live_review`+`llama_infer_bench`, comandos `dehyphenate_text`/`propose_heuristic_review` (core mantém a função), `apply_dehyphenate`, regexes órfãs, campos mortos de settings. Mantidos: exemplos de depuração (pipeline_dump/ocr_dump/seg_probe) e `next_accepts_letter_carry` (pendência carry).
5. ⏳ UAT do Zander nos livros reais antes de declarar pronto.

Rodada noturna (16/Ago, "corrija tudo" antes de dormir):
6. ✅ Causa do "texto cagado" (UAT noite): Tesseract emite `|` órfãos e linha em
   branco após CADA linha em páginas de entrelinha larga → cada linha virava
   parágrafo. Novo `normalize_ocr_page_text` (extraction.rs) + 4 testes.
   Prova: pág. 12 do Schopenhauer I sai em parágrafos reais.
7. ✅ Emissão dupla de página (vazia na passada nativa + real no OCR) eliminada —
   isso ERA o bug do teste `ocr_reconhece_paginas_do_livro_real`, que voltou a
   passar. Suite completa verde pela 1ª vez (76+1+1 core, 7 app).
8. ✅ UI: imagem com fit na moldura (object-fit contain, sem rolagem);
   texto ao vivo acompanha as páginas (gruda no fim, solta ao rolar p/ trás);
   rótulo "Bruto desta página" → "Texto melhorado página a página".
9. ✅ Novo exemplo de QA `page_melhorize_dump` (caminho ao vivo exato do app).
10. ✅ Convenção: builds oficiais via `_APP/scripts/build-release.sh` →
    `_APP/version/<data_hora>/TXTMelhorator.app` (memória + AGENTS.md).

Pendências conhecidas (não bloqueiam UAT): nota de rodapé OCR entra no fluxo do
corpo ("NI Este primeiro tomo…"); hífen na virada de página ("quar-" + "to").

## Não fazer (handover)

- Não voltar a "checklist Aplicar" nem "IA só no fim" (Zander rejeitou).
- Não rodar `tauri dev` / bench GGUF sem autorização.
- Não apagar goldens em `_temp/goldens/`.

## Depois do P0 (fila)

P1 U8–U10 (polish DS, fila/biblioteca, diff viewer) · P2 builds Linux/Windows + Releases · P3 hOCR tabelas.

Refs: `_docs/HANDOVER-2026-08-16-EOD.md` · `.agent/brain/EXPERIMENT_REVIEW_LIVE.md` · `_docs/BACKLOG-MELHORADOR.md`
