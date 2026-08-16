# TXTMelhorator — Visão para integração no SADE

Documento de referência para incorporar esta ferramenta ao ecossistema SADE (Z•Edições). Descreve o que faz, como executa e quais dependências usa. **Não usa IA em nenhuma etapa** (OCR clássico + heurísticas determinísticas + revisão humana opcional).

---

## 1. O que é

CLI local em Python que:

1. Extrai texto de PDFs de livros (texto nativo ou scan via OCR).
2. Limpa o texto de forma determinística (Unicode, hifenização de quebra, cabeçalhos/rodapés, espaços).
3. Prepara revisão ortográfica/gramatical via LanguageTool Premium **manual** (sem API na PoC atual).
4. Gera artefatos auditáveis (`raw`, `cleaned`, `report.json`, diffs).

**Não faz:** reescrita criativa, “completar” texto faltante, classificação por LLM, cloud OCR pago.

**Princípio:** fidelidade à fonte OCR/PDF. Só formatação e legibilidade.

---

## 2. Papel no SADE (contexto amplo)

| No Melhorador (hoje) | No SADE (futuro) |
|---|---|
| CLI local batch/faixa de páginas | Possível job/worker ou painel editorial |
| Entrada: PDF em `_ originais/` | Entrada: asset editorial / upload no hub |
| Saída: Markdown limpo + relatório | Saída: ingestão em pacote editorial / revisão |
| LanguageTool manual | Mesmo fluxo ou API Premium se houver chave |

Sugestão de fronteira: o Melhorador permanece **serviço de conversão local** (ou worker sem GPU/LLM). O SADE orquestra quem chama, onde guardar o PDF e o que fazer com o Markdown. Sem acoplar modelos de IA nesta etapa.

---

## 3. Pipeline

```
PDF
  → detectar camada de texto (pypdf)
  → se insuficiente: OCR (OCRmyPDF + Tesseract por+eng)
  → limpeza determinística (ftfy + heurísticas)
  → cleaned.md + report.json
  → (opcional) pacote LanguageTool → revisão humana → diff
```

Nenhuma chamada a LLM, embedding ou API generativa.

---

## 4. Como executar

### Pré-requisitos (macOS)

```bash
brew install python@3.12 tesseract tesseract-lang ghostscript qpdf unpaper
```

### Setup

```bash
cd "/Users/zander/Documents/_ coding/_ TXTMelhorator"
python3.12 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
```

### Comandos

```bash
# Extrair + limpar faixa de páginas
txtmelhorator extract \
  --input "_ originais/<arquivo>.pdf" \
  --pages 21-30 \
  --name <slug>

# Pacote para revisão no LanguageTool Premium (manual)
txtmelhorator prepare-lt \
  --input "_output/<slug>/pages-XXX-YYY/cleaned.md"

# Após salvar corrected.md no editor Premium
txtmelhorator import-lt \
  --original  "_output/<slug>/pages-XXX-YYY/languagetool/original.txt" \
  --corrected "_output/<slug>/pages-XXX-YYY/languagetool/corrected.md"
```

### Testes

```bash
python -m pytest
```

### Boundary operacional

- PoC valida faixas (ex.: 21–30). OCR do livro inteiro (centenas de páginas) exige autorização explícita (custo de CPU/tempo).
- PDFs e saídas (`_originais/`, `_output/`, `_temp/`) são **local-only** — fora do git.

---

## 5. Ferramentas e bibliotecas

### Runtime

| Peça | Função | Licença / nota |
|---|---|---|
| Python 3.12 | Runtime da CLI | — |
| `pypdf` | Recorte de páginas + texto nativo | BSD |
| `ocrmypdf` | Orquestra OCR em PDF scan | MPL-2.0 |
| Tesseract (`por+eng`) | Motor OCR | Apache-2.0 |
| Ghostscript / qpdf / unpaper | Deps nativas do OCRmyPDF (limpeza de imagem) | várias |
| `ftfy` | Correção de Unicode/mojibake | — |
| `pytest` | Testes | — |

### Serviços externos (opcional, humano)

| Peça | Função |
|---|---|
| LanguageTool Premium (editor) | Revisão ortográfica/gramatical **manual**; a PoC não chama a Proofreading API |

### Explicitamente fora do escopo

- LLMs (OpenAI, Anthropic, Ollama, etc.)
- Docling / Marker / pdfmux com backends neurais (podem ser reavaliados depois, mas **não** entram enquanto a regra for zero-IA)
- Cloud OCR pago

---

## 6. Artefatos de saída

```
_output/<doc>/pages-XXX-YYY/
├── raw.txt           # texto bruto (OCR ou nativo)
├── cleaned.md        # texto limpo + headings Markdown (#–####)
├── report.json       # engine, páginas, hashes, métricas de limpeza
└── languagetool/
    ├── original.txt
    ├── manifest.json   # SHA-256 + instruções (sem secrets)
    ├── corrected.md    # produzido pelo humano
    └── changes.diff
```

`report.json` não contém credenciais.

---

## 7. Estado da PoC vs gaps para o SADE

**Pronto**

- Extração nativa/OCR por faixa
- Limpeza determinística (Unicode, hífen de quebra, cabeçalhos/rodapés, espaços)
- Estrutura Markdown H1–H4 + bloco SUMÁRIO/ÍNDICE (heurísticas; sem IA)
- CLI + testes + relatório
- Fluxo LanguageTool manual auditável
- Documentação de integração SADE

**Pendente (backlog)**

- OCR integral sob autorização
- API LanguageTool (se houver `username` + `apiKey`)
- Empacotamento como worker/API consumível pelo SADE (contrato de I/O, filas, status)
- Afinar H1 em páginas de rosto/colofão e OCR de sumários quando existirem no PDF

---

## 8. Contrato sugerido para o SADE (rascunho)

Entrada mínima:

```json
{
  "pdf_path": "/abs/path/livro.pdf",
  "pages": "21-30",
  "languages": "por+eng",
  "doc_id": "mesopotamia"
}
```

Saída mínima:

```json
{
  "cleaned_md_path": ".../cleaned.md",
  "report_path": ".../report.json",
  "engine": "ocr|native",
  "status": "ok|error"
}
```

Implementação futura pode ser: subprocess da CLI atual, ou módulo Python importável (`txtmelhorator.extraction` / `cleanup`), sempre **sem** dependência de IA.

---

## 9. Referências no repositório

- Código: `src/txtmelhorator/`
- Uso rápido: [`README.md`](../README.md)
- Regras do agente: [`AGENTS.md`](../AGENTS.md)
- Backlog local: `_docs/BACKLOG.md` (não versionado)
- Mapa ZBOOKER (índices, citações, adoção): [`ZBOOKER_FERRAMENTAS.md`](ZBOOKER_FERRAMENTAS.md) → SSOT no SADE
