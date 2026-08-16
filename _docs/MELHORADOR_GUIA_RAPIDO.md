---
sistema: MELHORADOR
tipo: operacao
atualizado_em: 2026-08-16
---

# TXTMelhorator — Guia Rápido

## Comece agora

```bash
# 1. Vá para a pasta do projeto
cd "/Users/zander/Documents/_ coding/_ TXTMelhorator"

# 2. Roda tudo (setup + batch-extract + check-lt)
bash melhorar.sh
```

## O que o script `melhorar.sh` faz

1. **FASE 1: SETUP**
   - Verifica/instala deps nativas (python@3.12, tesseract, languagetool, etc.)
   - Cria/ativa Python venv
   - Instala dependências Python
   - Inicia servidor LanguageTool local na porta 8081

2. **FASE 2: BATCH-EXTRACT**
   - Descobre PDFs em `_originais/`
   - Extrai texto (OCR + nativo)
   - Limpa formatação e legibilidade
   - Gera saídas em `_output/<livro>/pages-XXX-YYY/`

3. **FASE 3: CHECK-LT**
   - Revisa cada `cleaned.md` com LanguageTool local
   - Gera sugestões (`lt-local-suggestions.json`)
   - Cria proposta com 1ª sugestão (`lt-local-corrected.md`)
   - Gera diff para aprovação (`lt-local-changes.diff`)

## Saídas

Após rodar, você terá:

```
_output/
├── <livro1>/pages-001-500/
│   ├── raw.txt                    # Texto bruto extraído
│   ├── cleaned.md                 # Texto limpo
│   ├── report.json                # Métricas (hashes, avisos)
│   └── languagetool/
│       ├── lt-local-suggestions.json    # Todas as ocorrências
│       ├── lt-local-corrected.md        # Proposta
│       └── lt-local-changes.diff        # Diff
│
├── <livro2>/pages-501-1000/
│   └── ...
```

## Próximos passos

1. **Revisar:** Abra `lt-local-corrected.md` e valide sugestões
2. **Aprovar:** Revise o `lt-local-changes.diff` antes de aplicar
3. **(Opcional) Premium:** Se tiver credenciais, use `import-lt` para revisão manual final

## Detalhes Técnicos

### Arquitetura Zero-IA

- ✅ **OCR:** Tesseract clássico (`por+eng`) — sem rede neural generativa
- ✅ **Limpeza:** Heurísticas determinísticas (ftfy, hifenização, headers)
- ✅ **Revisão:** LanguageTool (regras baseadas em padrões, sem ML generativo)
- ❌ Sem LLM, sem generativo, sem IA em nenhuma etapa

### LanguageTool Local

- Servidor HTTP em `localhost:8081` (privado, sem dados deixando a máquina)
- Automático de chunking para textos > 60KB
- Timeouts esperados em textos > 3M caracteres (ex: Paideia completo)
- Sem autenticação, sem API key — só regras locais

## Comandos Avançados

Se precisar rodar **só uma fase** (após o setup inicial):

### Apenas Batch-Extract
```bash
source .venv/bin/activate
txtmelhorator batch-extract --input-dir _originais --output-dir _output --temp-dir _temp
```

### Apenas Check-LT
```bash
source .venv/bin/activate
bash -c 'languagetool --http --port 8081 > /tmp/lt.log 2>&1 &'
sleep 3
txtmelhorator check-lt --input _output/<livro>/pages-XXX-YYY/cleaned.md
```

### Single (arquivo + páginas específicas)
```bash
source .venv/bin/activate
txtmelhorator extract --input "_originais/<arquivo>.pdf" --pages 1-50 --name meu_livro
```

## Logs & Debugging

- LanguageTool log: `/tmp/txtmelhorator-languagetool.log`
- Batch report: `_output/BATCH_REPORT.json`
- Per-book report: `_output/<livro>/pages-XXX-YYY/report.json`

Matar servidor LanguageTool manualmente:
```bash
pkill -f "languagetool --http"
```

## Limitações Conhecidas

1. **Paideia (1457 páginas, 3M+ chars):** LanguageTool pode sofrer timeout (server resource limit). Solução: processar em faixas menores ou usar a API Premium manual.
2. **PDFs com fontes embarcadas (non-OCR):** Se o PDF não tiver layer de texto, usa-se OCR (Tesseract) que pode fragmentar. Validar `report.json` para `quality_score` baixo.
3. **ISBN em capas:** Regex assume formato "978-XXXXXXXXXX" ou "978 XXXXXXXXXX". Outros formatos requerem ajuste manual no metadata.py.

## Suporte

- Documentação completa: `_docs/INDEX.md`
- Arquitetura: `_docs/arquitetura/AS_IS.md`
- LanguageTool: `_docs/integracoes/LANGUAGETOOL.md`
- Backlog: `_docs/BACKLOG.md`
