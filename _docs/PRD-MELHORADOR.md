---
sistema: MELHORADOR
tipo: prd
atualizado_em: 2026-08-16
---

# PRD — TXTMelhorator

**Produto:** ferramenta local que extrai texto de PDFs de livros digitalizados e melhora a conversão — formatação e legibilidade — **sem inventar conteúdo**.
**Superfícies:** App desktop (uso diário) e CLI Python (referência + lote no Mac de desenvolvimento).
**Versão:** PoC 0.2.0 · 16/Ago/2026 · uso local (Z•Edições).

Este é o **único PRD**. O texto de 01/Ago está em `_historico/2026-08-16_PRD-pre-pivo.md`.

---

## 1. Problema

Livros escaneados chegam como PDF de imagem. O texto extraído vem ilegível: OCR sujo, hífen quebrado, cabeçalho e número de página no meio do corpo, parágrafos fragmentados, Unicode corrompido. Revisar isso do zero é inviável no fluxo editorial.

## 2. Visão

PDF escaneado → **Markdown limpo, estruturado e auditável**, pronto para revisão humana e ingestão editorial.

A ferramenta é um faxineiro fiel, não um autor automático: **tudo o que sai existe na fonte**.

## 3. Duas superfícies, um produto

| | **App desktop** (`_APP/`) | **CLI** (`_CLI/`) |
|---|---|---|
| Papel | Uso diário | Referência de qualidade + lote no Mac de dev |
| Stack | Tauri 2 · UI React/TS · core Rust | Python 3.12 · OCRmyPDF · Tesseract |
| Entrada | **Alvo:** PDF ou pasta. **Hoje:** soltar um PDF | `_originais/` ou `extract --input` |
| Saída | **Alvo:** pergunta onde salvar (default = pasta do PDF). **Hoje:** ao lado, no clique | `_output/<slug>/` + `report.json` |
| Extração | PDFium + Tesseract (`por+eng`) | pypdf ou OCRmyPDF/Tesseract |
| Limpeza | Modo **aprimorado** (parágrafos, listas, sumário à francesa) | Modo **paridade** (golden master) |
| Revisão LT | ainda não | local (`check-lt`) + Premium manual |
| Distribuição | alvo: um `.app` sem instalar nada | Homebrew + `.venv` — aceitável só em dev |

O CLI **não some**. Ele congela o comportamento de referência. O core Rust prova paridade byte a byte nos goldens; o app pode ir além no modo aprimorado, com as diferenças documentadas no código.

## 4. Princípios (inegociáveis)

| Princípio | Prática |
|---|---|
| **Fidelidade** | Nunca completar, adivinhar ou reescrever o livro. Palavra ilegível permanece ilegível e é sinalizada. |
| **Zero-IA no pipeline** | Extração, limpeza e estrutura: OCR clássico (Tesseract) + regras fixas. Sem LLM, OCR neural generativo ou embeddings. |
| **Revisão só propõe** | LanguageTool, regras que o usuário ensinou e IA local geram diff. Nada entra no texto final sem o humano aceitar. |
| **Auditabilidade** | Hashes, métricas e avisos. CLI grava `report.json`. App ainda não replica esse contrato por completo. |
| **Obras fora do git** | PDFs e saídas em `_originais/`, `_output/`, `_temp/`. No app, origem e destino são escolhidos pelo usuário. |

**IA local (emenda 16/Ago):** permitida **só como revisão opt-in**, desligada por padrão, no aparelho. Não entra em extração, OCR, limpeza nem estrutura. Vocabulário do livro = termos extraídos do próprio texto (âncora na fonte). Modelo que inventar ou reescrever estilo é reprovado. Ver `AGENTS.md`.

**IA na nuvem (emenda 16/Ago — 3ª opção de revisão):** o usuário pode enviar o texto a uma API de sua preferência (formato OpenAI: URL + modelo + chave). Opt-in com aviso explícito de que o texto sai do computador. Continua só propondo diff.

## 5. Rotina do app (alvo de produto)

Decisão de ideação 16/Ago. **Primeiro uso diário = esta lista inteira.** Aprender = regras salvas primeiro; ajustar modelo só se as regras não bastarem.

### 5.1 Fluxo

```
Usuário abre o app
  → escolhe um PDF ou uma pasta com um ou mais PDFs
  → o app identifica o idioma (ou pergunta)
  → o app vê se já há texto nativo
      SIM → extrai Markdown e começa a melhorar
      NÃO → OCR página a página (progresso visível) e depois melhora
  → no fim, pergunta onde salvar (default = pasta do PDF original)
```

| Passo | Estado hoje |
|---|---|
| Abrir o app | ✅ |
| Um PDF | ✅ soltar na janela |
| Pasta / vários PDFs | ⬜ |
| Idioma: detectar ou perguntar | ⬜ fixo `por+eng` |
| Texto nativo vs OCR | ✅ limiar 200 chars |
| Extrair .md e melhorar (nativo) | 🔶 modo aprimorado, incompleto |
| OCR com progresso | ✅ |
| Perguntar onde salvar no **fim** | ⬜ salva ao lado, no clique |

### 5.2 Melhorar (nessa ordem)

| # | O quê | Estado hoje |
|---|---|---|
| M1 | Identificar idioma | ⬜ |
| M2 | Identificar colunas | ⬜ (acervo atual tratado como uma coluna) |
| M3 | Identificar parágrafos | 🔶 nativo aprimorado |
| M4 | Remover cabeçalhos, rodapés, nº de página | 🔶 |
| M5 | Marcar sumário, ficha técnica/catalográfica, bibliografia, imagens | 🔶 sumário/ficha parciais; imagens ⬜ |
| M6 | Concatenar parágrafos partidos (página e coluna) | 🔶 carry frágil; Paideia “era ne/cessário” ainda quebra |
| M7 | Regras que o usuário ensina (marca o que é cabeçalho, nota, etc.) | ⬜ preferências locais; **não** treina extração |
| M8 | IA local revisa o texto (só propõe diff) | ⬜ |
| M9 | Vocabulário do livro alimenta essa IA (termos da própria fonte) | ⬜ |
| M10 | Conferência lado a lado (página original \| texto) | ⬜ |

Construção, ainda com “tudo” no alvo: (1) pasta + idioma + salvar no fim → (2) layout → (3) lado a lado → (4) regras → (5) IA + vocabulário.

## 6. Usuário e contexto

- **Usuário:** Zander (Z•Edições), preparação de originais.
- **App:** janela no Mac; rotina da §5 (hoje: soltar um PDF, progresso, preview, salvar ao lado).
- **CLI:** terminal no Mac de desenvolvimento; lote em `_originais/`.
- **Acervo de referência:** Pierre Levêque (578 págs.), Paideia, Schopenhauer Tomos I–II. Outros PDFs na pasta ainda não entraram no lote.
- **Consumidor futuro (pendente):** hub SADE, como worker. Sem binding hoje.

## 7. Objetivos e metas

| Objetivo | Meta | Status |
|---|---|---|
| Extrair texto (nativo vs OCR) sem escolher na mão | Engine automático | ✅ CLI e App |
| Texto legível sem inventar conteúdo | QA em amostra + UAT nos nativos | ✅ com ressalvas (sumário, tabelas, notas) |
| Markdown com títulos e sumário, sem IA | H1–H4 + SUMÁRIO | 🔶; multilinha melhorou no app (Q3) |
| Livro inteiro | App processa o PDF todo; CLI tem `--full` | ✅ mecanismo; lote CLI já rodou 4 livros |
| Paridade CLI → Rust | Goldens dos 4 livros, byte a byte | ✅ modo paridade |
| Identificar obra (autor/título/ISBN) | Slug a partir da ficha | 🔶 só CLI; OCR sujo cai no nome do arquivo |
| Revisão gramatical auditável | LT local + Premium manual | 🔶 só CLI |
| App usável no dia a dia | Rotina §5 completa | 🔶 MVP: um PDF, sem pasta/idioma/salvar-no-fim |
| Binário sem Homebrew | Tesseract/tessdata no app | ⬜ pendente |
| Worker SADE | contrato I/O | ⬜ pendente |

## 8. Funcionalidades

### Comuns (pipeline)

| ID | O quê | CLI | App |
|---|---|---|---|
| F1 | Extração nativo/OCR (`por+eng`), limiar 200 chars | `extract` / batch | `process_pdf` |
| F2 | Limpeza: Unicode, hífen, cabeçalho, nº de página, reflow | paridade | aprimorado |
| F3 | Estrutura Markdown (H1–H4, sumário, filtro de créditos) | paridade | aprimorado |
| F4 | Avisos (ex.: caracteres `�`) | `report.json` | banner na janela |

### Só CLI

| ID | O quê | Comando |
|---|---|---|
| F5 | Pacote LanguageTool Premium (hash + chunks) | `prepare-lt` |
| F6 | Importar revisão → `changes.diff` (nada aplicado sozinho) | `import-lt` |
| F7 | Metadados da ficha → slug | `metadata.py` no batch |
| F8 | Lote: varre pasta, checkpoint, `BATCH_REPORT.json` | `batch-extract` / `_CLI/melhorar.sh` |
| F10 | LT **local** (offline) → sugestões + proposta + diff | `check-lt` |

### Só App

| ID | O quê | Estado |
|---|---|---|
| A1 | Soltar PDF/.txt/.md na janela | ✅ |
| A2 | Progresso página a página no OCR | ✅ |
| A3 | Preview com salto Início/Meio/Fim | ✅ |
| A4 | Salvar `.melhorado.md` / `.txt` ao lado da origem | ✅ |
| A5 | Modo aprimorado (cabeçalho nativo, listas, sumário à francesa, guarda de nota) | ✅ em código; montador v3 ainda aberto (ordem de linhas, palavra na virada de página) |

### Planejadas

| ID | O quê | Onde |
|---|---|---|
| A6 | Pasta com vários PDFs + fila | App |
| A7 | Idioma: detectar ou perguntar | App |
| A8 | Diálogo de destino no **fim** (default = pasta do PDF) | App |
| A9 | Exportar `.docx` com estilos | App |
| A10 | LT no app (Premium e/ou servidor local por URL) | App |
| A11 | Diff aceitar/rejeitar por ocorrência | App |
| A12 | Tesseract/Leptonica/tessdata dentro do binário | App |
| A13 | Colunas, imagens, ficha/bib como blocos | App |
| A14 | Regras que o usuário ensina (arquivo local) | App |
| A15 | Conferência lado a lado (página \| texto) | App |
| A16 | IA local + vocabulário do livro (só revisão) | App |
| F9 | Faixa de páginas e `--full` com confirmação na UI | App (CLI já tem) |
| F11 | Faixas não contíguas (`1-10,50-60`) | CLI |
| F12 | Dicionário determinístico de typos de OCR | os dois |
| F13 | Contrato worker SADE | pendente de produto |
| F14 | API LanguageTool Premium na nuvem | se houver chave |
| Q1 | Tabelas → Markdown (hoje viram prosa) | os dois |
| Q2 | Notas de rodapé realocadas (hoje no meio do texto) | os dois |
| Q3 | Sumário multilinha sem virar título falso | 🔶 aprimorado no app (16/Ago); CLI paridade intacta |

## 9. Requisitos não funcionais

- **Determinismo:** mesma entrada → mesma saída no pipeline. Nenhuma etapa aleatória.
- **Fail-fast:** primeira falha para e explica (CLI: exit 1).
- **CLI:** Python 3.12 no `_CLI/.venv`; nativos via Homebrew (Tesseract, Ghostscript, qpdf, unpaper, LanguageTool).
- **App (alvo):** um artefato por sistema, sem o usuário instalar Python/Java/brew. **Hoje** o OCR ainda usa Tesseract/Leptonica do Homebrew e PDFium no bundle.
- **Testes:** CLI 61 pytest; core Rust 33 (`cargo test --release`); goldens dos 4 livros. Teste de máquina ≠ teste de UI.
- **Licenças de bundle:** sem Ghostscript (AGPL) no app. PDFium BSD; Tesseract Apache 2.0.

## 10. Métricas de sucesso

- Zero conteúdo inventado na revisão (diff como evidência).
- Suíte verde: 61 Python + 33 Rust.
- Goldens: 4 livros, 7,9 M caracteres, paridade no modo CLI.
- Lote CLI: 4/4 livros processados (Paideia: LT local estourou tempo).
- UAT do app: o caso que o Zander apontou tem de fechar com prova (página + trecho), não com “deveria funcionar”.

## 11. Fora de escopo

- Reescrever o autor (estilo, “melhorar” o texto).
- OCR pago na nuvem.
- LLM na extração, limpeza ou estrutura.
- Treinar modelo **no pipeline** de extração (aprender = regras salvas; modelo só na revisão, se as regras não bastarem).
- EPUB final (ZBOOKER).
- Substituir o revisor humano.
- Mobile.
- Worker HTTP / API própria (até haver decisão SADE). A **fila na janela** do app está **dentro** do escopo (§5).
