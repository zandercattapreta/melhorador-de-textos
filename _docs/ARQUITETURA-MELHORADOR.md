---
sistema: MELHORADOR
tipo: arquitetura
atualizado_em: 2026-08-16
---

# Arquitetura — TXTMelhorator

Sistemas, módulos, dependências e fluxo de dados. Referências: [`PRD-MELHORADOR.md`](PRD-MELHORADOR.md) (§5 rotina) · [`BACKLOG-MELHORADOR.md`](BACKLOG-MELHORADOR.md) (R1–R5) · estado em [`arquitetura/AS_IS.md`](arquitetura/AS_IS.md).

---

## 1. Forma geral

Uma ferramenta, **duas superfícies**. Sem servidor próprio, sem banco.

| Superfície | Onde | Papel |
|---|---|---|
| **App** | `_APP/` — Tauri 2, UI React, core Rust | Uso diário (alvo: rotina PRD §5) |
| **CLI** | `_CLI/` — Python 3.12 | Referência (goldens) + lote no Mac de dev |

O app **não** chama o Python em runtime. O CLI congela o comportamento de referência.

**Pipeline (extração → limpeza → estrutura): zero LLM.** Revisão (LT / regras / IA local) só **propõe** diff; o humano aceita.

---

## 2. App — estrutura-alvo (PRD)

Mapa do que o produto **deve** usar. Ordem de construção: R1 → R5 no backlog.

```
┌──────────────────────────────────────────────────┐
│  UI  React + Tauri                               │
│  pasta/PDF · idioma · fila · lado a lado · diff  │
└────────────────────┬─────────────────────────────┘
                     │ invoke / events
┌────────────────────▼─────────────────────────────┐
│  Core Rust (determinístico — SEM LLM)            │
│  PDFium → Tesseract → cleanup → structure        │
│  + layout (colunas, blocos, juntas)              │
│  + regras do usuário                             │
└──────────┬───────────────────┬───────────────────┘
           │                   │
     ┌─────▼─────┐       ┌─────▼──────────────┐
     │ Revisão   │       │ Dados locais       │
     │ LT e/ou   │       │ preferências ·     │
     │ IA local  │       │ vocab do livro ·   │
     │ (só diff) │       │ modelos GGUF       │
     └───────────┘       └────────────────────┘
```

### 2.1 Motores (pipeline)

| Peça | Papel | Hoje | Alvo |
|---|---|---|---|
| **PDFium** (`pdfium-render` + dylib) | Abrir PDF, texto nativo, render de página | ✅ no bundle | ✅ |
| **Tesseract + Leptonica** (`leptess`) | OCR | 🔶 Homebrew | ✅ estático no binário + tessdata (A12) |
| **Pré-processamento de imagem** | Cinza / binarizar / deskew | 🔶 só cinza | ✅ Rust próprio (sem Ghostscript) |
| **Detector de idioma** | Heurística ou pergunta → idiomas do OCR | ⬜ fixo `por+eng` | ✅ (A7 / M1) |
| **extraction** | Nativo vs OCR (limiar 200 chars); progresso página a página | ✅ | ✅ + faixa/`--full` na UI |
| **cleanup** | Unicode, hífen, cabeçalho, nº página, reflow | ✅ aprimorado | ✅ |
| **structure** | H1–H4, sumário, listas | ✅ aprimorado | ✅ |
| **layout** | Colunas, juntas página/coluna, blocos (sumário/ficha/bib/imagens), tabelas/notas | 🔶 parcial | ✅ (A13, R2) |

### 2.2 Revisão (aplica + Desfazer)

LanguageTool e/ou IA geram diffs e a UI **aplica** no texto; lista o que mudou; **Desfazer** restaura o snapshot anterior. Sem inventar conteúdo.

| Peça | Papel | Hoje | Alvo |
|---|---|---|---|
| **Regras do usuário** | Preferências locais (marca cabeçalho, nota…) — **antes** do modelo | ⬜ | ✅ (A14 / M7) |
| **LanguageTool** | Gramática | ⬜ no app (só CLI) | ✅ Premium (keychain) e/ou servidor local por URL — **não** embute Java (A10) |
| **IA local** (`llama.cpp` + GGUF) | Revisão opt-in no aparelho | ⬜ | ✅ + vocabulário do livro âncora na fonte (A16 / M8–M9) |
| **Viewer de diff** | Aceitar / rejeitar por ocorrência | ⬜ | ✅ (A11) |

### 2.3 Casca / SO

| Peça | Papel | Hoje | Alvo |
|---|---|---|---|
| **UI React** | Dropzone, preview, progresso | ✅ um PDF | ✅ pasta, fila, lado a lado, config |
| **Tauri 2** | Ponte UI ↔ core | ✅ | ✅ |
| **Diálogos nativos** (`plugin-dialog`) | Abrir PDF/pasta; salvar no fim | ⬜ npm sem Rust | ✅ (A6, A8); default destino = pasta do PDF |
| **Fila** | Vários livros, progresso, cancelar | ⬜ | ✅ (A6) — **na janela**, não API HTTP |
| **Keychain** | Credenciais LT Premium | ⬜ | ✅ |
| **App data do SO** | Temp OCR, tessdata, modelos | ⬜ | ✅ |
| **Export** | `.md` / `.txt` / `.docx` | 🔶 md+txt ao lado | ✅ diálogo no fim + docx (A9) |
| **Conferência lado a lado** | Página raster \| texto | ⬜ | ✅ (A15 / M10) |

### 2.4 Módulos Rust / TS (hoje)

| Peça | Papel |
|---|---|
| `_APP/src/App.tsx` | Dropzone, progresso OCR, preview, salvar |
| `_APP/src-tauri/` | Comandos: `process_pdf`, `process_text_file`, `save_result`; evento `extract-progress` |
| `_APP/core/extraction.rs` | PDFium + Tesseract; montador nativo v3 |
| `_APP/core/cleanup.rs` | Limpeza (paridade + aprimorado) |
| `_APP/core/structure.rs` | Markdown (paridade + aprimorado) |
| `_APP/core/pystr.rs` · `pydifflib.rs` | Paridade com o CLI Python |

Modos: **paridade** (= CLI / goldens) · **aprimorado** (janela).

### 2.5 O que o app **não** embute (por decisão)

- Servidor HTTP / API / fila na nuvem  
- OCR pago na nuvem / Ghostscript (AGPL)  
- LLM na extração, limpeza ou estrutura  
- Java / LanguageTool embutido  
- Ollama como dependência (inferência = `llama.cpp` in-process)  
- Worker SADE (visão futura; ver §6)

---

## 3. CLI — lote e referência

Pacote Python (`pip install -e .`), console script `txtmelhorator`. Entrada: `_CLI/melhorar.sh` → `batch-extract` (padrão amostra 1–50; `--full` = livro inteiro).

```
PDF (_originais/)
  → extraction (pypdf ou OCRmyPDF/Tesseract)
  → cleanup → structure
  → _output/<slug>/…/{raw.txt, cleaned.md, report.json}
  → check-lt (LT local) e/ou prepare-lt / import-lt (Premium manual)
```

### 3.1 Módulos (`_CLI/src/txtmelhorator/`)

| Módulo | Responsabilidade |
|---|---|
| `cli.py` | `extract` / `batch-extract` / `prepare-lt` / `import-lt` / `check-lt` |
| `extraction.py` | Faixa de páginas; nativo (pypdf) ou OCR (OCRmyPDF) |
| `cleanup.py` | Limpeza determinística (9 passos) |
| `structure.py` | H1–H4, sumário, anti-colofão |
| `metadata.py` | Autor/título/ISBN → slug |
| `batch_extract.py` | Lote, checkpoint, `BATCH_REPORT.json` |
| `languagetool_review.py` | Pacote Premium + diff |
| `languagetool_local.py` | Servidor LT local (localhost:8081) |

### 3.2 Dependências CLI

| Peça | Papel |
|---|---|
| Python 3.12 · `pypdf` · `ocrmypdf` · `ftfy` · `pytest` | Runtime e testes |
| Homebrew: `tesseract`, `ghostscript`, `qpdf`, `unpaper`, `languagetool` | OCR e LT local |

Ambiente: `_CLI/.venv`. Não misturar com o Python do sistema.

---

## 4. Layout de pastas

```
_CLI/          # Python (referência + lote)
_APP/          # Tauri: src/ (UI) · core/ (Rust) · src-tauri/ (casca + PDFium)
_docs/         # documentação (hoje fora do git)
_originais/    # PDFs (local-only) — convenção do CLI; o app pergunta origem
_temp/         # lixo de OCR / goldens (local-only)
_output/       # saídas do CLI (local-only)
```

**App (alvo):** origem e destino escolhidos pelo usuário; intermediários em app-data do SO (`~/Library/Application Support/…`, etc.).

---

## 5. Decisões de arquitetura

| Decisão | Racional |
|---|---|
| Core Rust no Tauri, sem sidecar Python | Um binário; assinatura única; requisito “sem deps externas” no usuário final |
| PDFium + Tesseract (não OCRmyPDF/Ghostscript) | Licença; portabilidade 3 SOs |
| Limpeza/estrutura por regras | Determinismo; zero LLM no pipeline |
| Dois modos no core (paridade × aprimorado) | Goldens intactos; app pode ir além do CLI |
| Revisão só-diff | Fidelidade; humano decide |
| IA local só revisão + vocab da fonte | Emenda AGENTS 16/Ago; não inventa conteúdo |
| LT = nuvem Premium ou URL local | Java não entra no bundle |
| Fila **na janela**, não API HTTP | Produto desktop; SADE fica em visão |

---

## 6. Integrações

| Sistema | Superfície | Estado |
|---|---|---|
| **LanguageTool local** | CLI | Automático (brew, :8081) |
| **LanguageTool Premium** | CLI manual; app alvo | App: API + keychain (A10) |
| **IA local (GGUF)** | App alvo | Opt-in; `llama.cpp` in-process (A16) |
| **SADE** | Visão | Contrato I/O possível; sem binding |
| **ZBOOKER** | Fora | EPUB não é papel deste projeto |

---

## 7. O que não existe (por decisão)

- Worker HTTP / API própria / fila na nuvem  
- Cloud OCR pago  
- LLM em extração, limpeza ou estrutura  
- Persistência além de filesystem / app-data / keychain  
- Aplicação automática de correções (tudo passa por diff revisável)
