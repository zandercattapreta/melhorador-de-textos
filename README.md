# TXTMelhorator

Ferramenta local: extrai e limpa texto de PDFs de livros digitalizados, **sem inventar conteúdo**. OCR clássico + regras + revisão humana. Sem LLM no pipeline.

- **App** ([`_APP/`](_APP/)): janela Tauri — soltar o PDF, acompanhar página a página, salvar `.melhorado.md`
- **CLI** ([`_CLI/`](_CLI/)): referência de qualidade + lote no Mac de desenvolvimento

PRD: [`_docs/PRD-MELHORADOR.md`](_docs/PRD-MELHORADOR.md).

## Estado

**PoC 0.2.0** (16/Ago):
- ✅ App: PDF → OCR/nativo → Markdown; exporta `.md`/`.txt`
- ✅ CLI: batch-extract + metadados + LanguageTool local
- ✅ Testes: 61 pytest + 33 cargo (`--release`); goldens dos 4 livros
- ✅ Lote CLI: 4 PDFs reais (3,5 mil páginas)

**Zero-IA no pipeline.** Revisão LanguageTool só no CLI hoje.

## Stack

- App: Rust 1.97 · Tauri 2 · React 19 · PDFium + Tesseract (`por+eng`)
- CLI: Python 3.12 (`_CLI/.venv`) · OCRmyPDF · pypdf · ftfy · pytest

## Instalação & Pipeline Completo

> **Estrutura (15/Ago):** código do CLI em [`_CLI/`](_CLI/) · app desktop (Tauri 2 + core Rust) em [`_APP/`](_APP/) · dados na raiz (`_originais/`, `_output/`).

**Opção 1: Script automático (recomendado)**
```bash
bash _CLI/melhorar.sh
```
Faz tudo: setup deps → batch-extract → check-lt com LanguageTool local.

**Opção 2: Manual**
```bash
# Só setup (deps + venv + LanguageTool)
bash _CLI/setup.sh
```

**Opção 3: Comandos individuais**
```bash
# dependências nativas (macOS / Homebrew)
brew install python@3.12 tesseract tesseract-lang ghostscript qpdf unpaper languagetool

# ambiente Python (em _CLI/)
cd _CLI
python3.12 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
cd ..   # comandos do CLI rodam da raiz (dados em _originais/ e _output/)

# iniciar LanguageTool local (porta 8081)
languagetool --http --port 8081 &
```

## Uso

### Pipeline Completo (recomendado)

```bash
bash _CLI/melhorar.sh
```

**O que faz:**
1. ✅ Setup: verifica/instala deps, inicia LanguageTool local
2. ✅ Batch-extract: PDFs → `cleaned.md` + metadados
3. ✅ Check-LT: review automático com LanguageTool (localhost:8081)
4. ✅ Saídas: `lt-local-suggestions.json`, `lt-local-corrected.md`, `lt-local-changes.diff`

**Exemplo:**
```bash
$ bash _CLI/melhorar.sh
[INFO] Verificando deps nativas...
[✓] Todas as deps nativas OK
...
[✓] LanguageTool OK (PID 12345)
[✓] Batch-extract completo
[✓] Check-lt completo para todos os livros
```

### Batch Extract (manual)

```bash
source _CLI/.venv/bin/activate

txtmelhorator batch-extract \
  --input-dir _originais \
  --output-dir _output \
  --temp-dir _temp \
  --retry 1
```

### Single (legado — faixa de páginas)

```bash
# 1. Extrair + limpar uma faixa de páginas (gera raw.txt, cleaned.md, report.json)
txtmelhorator extract \
  --input "_originais/<arquivo>.pdf" \
  --pages 21-30 \
  --name mesopotamia

# 2. Gerar pacote de revisão do LanguageTool (original.txt + manifest.json)
txtmelhorator prepare-lt \
  --input "_output/mesopotamia/pages-021-030/cleaned.md"

# 3. (manual) Revisar no editor Premium do LanguageTool em pt-BR
#    Salve o texto revisado como corrected.md na pasta languagetool/

# 4. Importar o corrigido e gerar o diff auditável (changes.diff)
txtmelhorator import-lt \
  --original   "_output/mesopotamia/pages-021-030/languagetool/original.txt" \
  --corrected  "_output/mesopotamia/pages-021-030/languagetool/corrected.md"
```

### LanguageTool (assinante Premium)

A assinatura Premium do app/extensão não inclui, por padrão, credenciais da Proofreading API (`username` + `apiKey`). Por isso a PoC usa **revisão manual auditável**: o texto vai para o editor Premium e volta como `corrected.md`; o `import-lt` gera o diff sem aplicar sugestões sozinho. Se/quando houver chave da API, dá para automatizar via `https://api.languagetoolplus.com/v2/check`.

## Testes

```bash
cd _CLI && source .venv/bin/activate && python -m pytest   # CLI Python (61)
cd _APP/core && cargo test                                  # core Rust (port)
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
| `PRD-MELHORADOR.md` | PRD único (App + CLI) |
| `arquitetura/AS_IS.md` | Estado real (61 testes, 4 PDFs validados, metadados) |
| `BACKLOG-MELHORADOR.md` | Fila R1→R5 (app) |
| `integracoes/LANGUAGETOOL.md` | Fluxo manual + batch + chunking automático |

**Regra de Ouro:** extração/OCR/limpeza **sem** LLM. Revisão IA local só se opt-in (ver `AGENTS.md`).
- ✅ OCR: Tesseract clássico (`por+eng`)
- ✅ Limpeza: heurísticas determinísticas (ftfy, hifenização, headers)
- ✅ Estrutura: regras simples (H1–H4 por padrão texto, não ML)
- ✅ Revisão: LanguageTool Premium (humano revisa cada sugestão)

## Licença

MIT © 2026 Zander Catta Preta
