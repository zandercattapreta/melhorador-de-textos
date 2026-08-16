---
sistema: MELHORADOR
tipo: sessao
atualizado_em: 2026-08-16
---

# Plano — R3b (sync texto ↔ página)

## Objetivo

Ao mudar a página do PDF, o painel de texto mostra (ou rola até) o trecho correspondente.

## Prova técnica (hoje)

- Extração une páginas com `\f`; limpeza/estrutura **misturam** e o Markdown final quase não preserva fronteiras.
- UI R3a: raster muda; texto = documento inteiro.

## Escopo deste gate (R3b)

| # | Entrega |
|---|---|
| 1 | Pipeline enhanced devolve também `pages: Vec<String>` — texto **por página** (limpo+estrutura **por fatia** `\f`, sem fundir páginas) |
| 2 | `ProcessResult.pages` + `page_count` alinhados |
| 3 | UI: painel direito = `pages[confPage-1]` (fallback: doc inteiro se `pages` vazio) |
| 4 | Toggle opcional: **Página** \| **Livro inteiro** |
| 5 | Teste: N `\f` → N fatias após enhanced |

## Fora deste gate

- Highlight na imagem / bounding boxes
- Mapear aviso → página exata (heurística extra)
- R4 regras

## Como (curto)

- Nova fn `clean_and_structure_pages(raw) -> (Vec<String>, stats agregados)` no core.
- Cada fatia: `clean_text_enhanced` + `apply_structure_enhanced` + `annotate_blocks` (por página).
- `clean_and_structure_enhanced` atual continua para export `.md` do livro inteiro (como hoje).
- App: `process_pdf` preenche `pages` e `cleaned` (join com `\n\n`).

## Arquivos

| Arquivo | Mudança |
|---|---|
| `_APP/core/src/lib.rs` | `clean_and_structure_pages` |
| `_APP/src-tauri/src/lib.rs` | `pages` em `ProcessResult` |
| `_APP/src/App.tsx` (+ CSS) | toggle Página/Livro; texto da página |
| `_docs/BACKLOG-MELHORADOR.md` | R3b ✅ |

## Testes

- Core: 3 páginas sintéticas com `\f` → 3 strings
- `npm run build` + `cargo test` app
- Goldens: **não** tocar (paridade usa `clean_and_structure` antigo)

## Checklist

- Não inventar texto; só fatiar o que veio da fonte
- Export `.md` = livro inteiro (não só a página visível)
