# Melhorador de Textos

Extrai texto de PDFs de livros digitalizados e melhora a conversão (formatação e legibilidade), sem inventar conteúdo.

## Estado

PoC funcional: extração (nativa/OCR) + limpeza determinística + headings Markdown (H1–H4/SUMÁRIO, sem IA) + fluxo manual LanguageTool. Amostra validada: páginas **1–50** do Mesopotâmia.

## Stack

- Python 3.12 (`.venv`)
- [OCRmyPDF](https://github.com/ocrmypdf/OCRmyPDF) + Tesseract (`por+eng`), Ghostscript, qpdf, unpaper
- `pypdf` (recorte + texto nativo), `ftfy` (conserto de Unicode)
- `pytest`

## Instalação

```bash
# dependências nativas (macOS / Homebrew)
brew install python@3.12 tesseract tesseract-lang ghostscript qpdf unpaper

# ambiente Python
python3.12 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
```

## Uso

```bash
source .venv/bin/activate

# 1. Extrair + limpar uma faixa de páginas (gera raw.txt, cleaned.md, report.json)
melhorador-textos extract \
  --input "_ originais/<arquivo>.pdf" \
  --pages 21-30 \
  --name mesopotamia

# 2. Gerar pacote de revisão do LanguageTool (original.txt + manifest.json)
melhorador-textos prepare-lt \
  --input "_output/mesopotamia/pages-021-030/cleaned.md"

# 3. (manual) Revisar no editor Premium do LanguageTool em pt-BR e salvar
#    o texto revisado como corrected.md na pasta languagetool/

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

## Integração SADE

Visão, pipeline, ferramentas e contrato sugerido: [`docs/INTEGRACAO_SADE.md`](docs/INTEGRACAO_SADE.md).

Mapa ZBOOKER (índices, citações, adoção futura): [`docs/ZBOOKER_FERRAMENTAS.md`](docs/ZBOOKER_FERRAMENTAS.md).

**Regra:** esta ferramenta **não usa IA** (sem LLM/OCR neural generativo) — só OCR clássico, heurísticas e revisão humana opcional (LanguageTool).
