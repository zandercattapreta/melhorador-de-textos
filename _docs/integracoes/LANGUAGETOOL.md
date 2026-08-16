---
sistema: MELHORADOR
tipo: integracao
atualizado_em: 2026-08-15
---

# LanguageTool — Fluxo de Revisão

Revisão **híbrida** (15/Ago): camada automática no LT **local** + passada final **humana** no editor Premium. Zero-IA (regras determinísticas; respeita princípio do projeto).

## App desktop (`_APP/`)

Fluxo na janela (após processar o PDF):

1. **Revisar com LanguageTool** — sobe o servidor local se preciso (`brew install languagetool`), sugere correções, você marca e **Aplicar marcadas**.
2. **Revisar com IA local (opt-in)** — GGUF no aparelho; inferência **dentro do app** (`llama.cpp` embutido, Metal no Mac). Sem Ollama/`llama-cli`.
3. **IA na nuvem (opt-in)** — API no formato OpenAI (URL + modelo + chave); aviso de que o texto sai do Mac.
4. **LanguageTool Premium (nuvem)** — aviso explícito; texto sai da máquina.
5. **Salvar** — grava o texto e `*.report.json` com as correções aceitas.

Nada entra no texto sem o humano aceitar.


Servidor Java local (`brew install languagetool`, `localhost:8081`); o texto **não sai da máquina**. O batch (ou `check-lt --input cleaned.md`) submete cada livro e grava em `languagetool/`:

- `lt-local-suggestions.json` — todas as ocorrências apontadas;
- `lt-local-corrected.md` — proposta (1ª sugestão de cada ocorrência);
- `lt-local-changes.diff` — diff para aprovação humana.

Nada é aplicado ao `cleaned.md` sem aprovação. Servidor ausente → batch avisa e segue (`--no-lt` desliga a etapa). Limite: regras da versão gratuita — as regras Premium só existem na nuvem, por isso a passada manual continua.

O app "LanguageTool for Desktop" (macOS) **não expõe API local** — verificado em 15/Ago; não serve para automação.

## Fluxo Batch (Novo — 15/Ago)

```
batch-extract _originais → 4 PDFs
  ↓
4 × prepare-lt
  → _output/
     └─ <livro>/pages-XXX-YYY/
        └─ languagetool/
           ├─ original.txt (1.4M, Paideia; 1.1M, Schopenhauer; etc.)
           └─ manifest.json (chunks, hash, instruções pt-BR)
  ↓
[Revisão paralela no editor Premium — 4 revisores?]
  ↓
4 × import-lt
  → changes.diff por livro
```

**Chunking:** Manifest automaticamente divide textos > 60KB em chunks (ex: Paideia → 62 chunks, Schopenhauer II → 79 chunks).

## Fluxo Single (Legado)

1. `extract --input <pdf> --pages 21-30 --name <slug>`
2. `prepare-lt --input _output/<slug>/pages-021-030/cleaned.md`
3. [Usuário cola em Premium, salva corrected.md]
4. `import-lt --original ... --corrected ...` → diff

## Manual vs API

| Aspecto | Manual | API |
|---|---|---|
| Setup | Editor Premium (assinante) | `username` + `apiKey` (não temos) |
| Escala | 4 livros = 4 revisores em paralelo | Fila HTTP (custo/rate-limit) |
| Confiança | Humano revisa **cada** sugestão | Automático (risco de erro) |
| Projeto | Alinhado (zero-IA, auditável) | Questionável (confiabilidade) |

**Status:** Manual está pronto. API requer credenciais + análise de custo/benefício (P1, BACKLOG).
