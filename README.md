# Melhorador de Textos

**CLI escalável** que processa N PDFs de livros digitalizados, extrai e limpa texto (formatação/legibilidade), sem IA/LLM — apenas OCR clássico + heurísticas + revisão humana.

## Estado

**PoC 0.2.0** funcional:
- ✅ Batch-extract: descoberta automática + processamento sequencial de N PDFs
- ✅ Metadados: extração de autor/título/ISBN com confiança (0.55–0.95)
- ✅ Pipeline: extração (nativa/OCR) → limpeza → estrutura Markdown → LanguageTool
- ✅ Testes: 61 tests (pipeline + metadata), 100% sucesso
- ✅ Validação: 4 PDFs reais (3.5K páginas), BATCH_REPORT.json

**Zero-IA:** Tesseract clássico + regras determinísticas + LT Manual Premium (sem API automática).

## Stack

- Python 3.12 (`.venv`)
- [OCRmyPDF](https://github.com/ocrmypdf/OCRmyPDF) + Tesseract (`por+eng`), Ghostscript, qpdf, unpaper
- `pypdf` (recorte + texto nativo), `ftfy` (conserto de Unicode)
- `pytest`

## Instalação & Pipeline Completo

**Opção 1: Script automático (recomendado)**
```bash
bash melhorar.sh
```
Faz tudo: setup deps → batch-extract → check-lt com LanguageTool local.

**Opção 2: Manual**
```bash
# Só setup (deps + venv + LanguageTool)
bash setup.sh
```

**Opção 3: Comandos individuais**
```bash
# dependências nativas (macOS / Homebrew)
brew install python@3.12 tesseract tesseract-lang ghostscript qpdf unpaper languagetool

# ambiente Python
python3.12 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"

# iniciar LanguageTool local (porta 8081)
languagetool --http --port 8081 &
```

## Uso

### Pipeline Completo (recomendado)

```bash
bash melhorar.sh
```

**O que faz:**
1. ✅ Setup: verifica/instala deps, inicia LanguageTool local
2. ✅ Batch-extract: PDFs → `cleaned.md` + metadados
3. ✅ Check-LT: review automático com LanguageTool (localhost:8081)
4. ✅ Saídas: `lt-local-suggestions.json`, `lt-local-corrected.md`, `lt-local-changes.diff`

**Exemplo:**
```bash
$ bash melhorar.sh
[INFO] Verificando deps nativas...
[✓] Todas as deps nativas OK
...
[✓] LanguageTool OK (PID 12345)
[✓] Batch-extract completo
[✓] Check-lt completo para todos os livros
```

### Batch Extract (manual)

```bash
source .venv/bin/activate

melhorador-textos batch-extract \
  --input-dir _originais \
  --output-dir _output \
  --temp-dir _temp \
  --retry 1
```

### Single (legado — faixa de páginas)

```bash
# 1. Extrair + limpar uma faixa de páginas (gera raw.txt, cleaned.md, report.json)
melhorador-textos extract \
  --input "_originais/<arquivo>.pdf" \
  --pages 21-30 \
  --name mesopotamia

# 2. Gerar pacote de revisão do LanguageTool (original.txt + manifest.json)
melhorador-textos prepare-lt \
  --input "_output/mesopotamia/pages-021-030/cleaned.md"

# 3. (manual) Revisar no editor Premium do LanguageTool em pt-BR
#    Salve o texto revisado como corrected.md na pasta languagetool/

# 4. Importar o corrigido e gerar o diff auditável (changes.diff)
melhorador-textos import-lt \
  --original   "_output/mesopotamia/pages-021-030/languagetool/original.txt" \
  --corrected  "_output/mesopotamia/pages-021-030/languagetool/corrected.md"
```

### LanguageTool (assinante Premium)

A assinatura Premium do app/extensão não inclui, por padrão, credenciais da Proofreading API (`username` + `apiKey`). Por isso a PoC usa **revisão manual auditável**: o texto vai para o editor Premium e volta como `corrected.md`; o `import-lt` gera o diff sem aplicar sugestões sozinho. Se/quando houver chave da API, dá para automatizar via `https://api.languagetoolplus.com/v2/check`.

## Testes

```bash
source .venv/bin/activate
python -m pytest
```

## Saídas (local-only, fora do git)

```
_output/<doc>/pages-XXX-YYY/
├── raw.txt                    # texto bruto (OCR ou nativo)
├── cleaned.md                 # texto limpo
├── report.json                # métricas, hashes, avisos
└── languagetool/
    ├── original.txt           # texto a colar no LanguageTool
    ├── manifest.json          # hash + instruções + chunking
    ├── corrected.md           # (manual) texto revisado
    └── changes.diff           # diff original -> corrigido
```

## Agente

- Canônico: `AGENTS.md`
- Workflows: `/sod` `/eod` `/eow` em `.agent/workflows/`
- Skills: `.agent/skills/golden-rules`, `.agent/skills/dod`

## Documentação (em `_docs/`)

| Arquivo | Conteúdo |
|---|---|
| `INDEX.md` | Mapa do projeto |
| `PRD.md` | Princípios (fidelidade, zero-IA, auditável) |
| `arquitetura/AS_IS.md` | Estado real (61 testes, 4 PDFs validados, metadados) |
| `BACKLOG.md` | P0–P2 (docs versionadas, remoto, API LanguageTool) |
| `integracoes/LANGUAGETOOL.md` | Fluxo manual + batch + chunking automático |

**Regra de Ouro:** sem IA/LLM em nenhuma etapa.
- ✅ OCR: Tesseract clássico (`por+eng`)
- ✅ Limpeza: heurísticas determinísticas (ftfy, hifenização, headers)
- ✅ Estrutura: regras simples (H1–H4 por padrão texto, não ML)
- ✅ Revisão: LanguageTool Premium (humano revisa cada sugestão)

## Licença

MIT © 2026 Zander Catta Preta
