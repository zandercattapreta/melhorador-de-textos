---
sistema: PROJETO
tipo: politica
atualizado_em: 2026-08-01
---

# Política de Documentação

Validação: `docs.config.json` + `bash "../~scripts/docs/check-docs.sh" .`.

## 1. Regras

1. Um tema, um documento.
2. Doc a partir do código e dos `report.json` — não inventar etapas de IA.
3. Durável ≠ efêmero (QA de amostra → `_sessoes/` ou `_historico/`).
4. `INDEX.md` é contrato.
5. Front-matter obrigatório.
6. Rotação → `_historico/`.

## 2. Fora de `_docs/` (e do git)

| Path | Natureza |
|---|---|
| `_ originais/`, `*.pdf` | obras — local-only |
| `_output/`, `_temp/` | artefatos de execução |
| `.venv/` | ambiente |

## 3. `_docs/` está no `.gitignore`

Dívida: [AS_IS §7](arquitetura/AS_IS.md). Docs existem no disco e em `_bkps/`; o git do pacote Python **não** os versiona com a config atual.

## 4. Taxonomia

| Diretório | Conteúdo |
|---|---|
| raiz | índice, política, glossário, PRD, roadmap, backlog |
| `arquitetura/` | AS IS, arquitetura |
| `integracoes/` | SADE (visão), LanguageTool |
| `operacao/` | CLI, deps nativas |
| `_historico/` | backlog/QA/changelog anteriores |

## 5. Front-matter

```yaml
---
sistema: PROJETO | MELHORADOR
tipo: indice | politica | glossario | prd | roadmap | backlog | arquitetura | as-is | integracao | operacao | sessao
atualizado_em: AAAA-MM-DD
---
```
