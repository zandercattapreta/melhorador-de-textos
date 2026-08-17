---
sistema: MELHORADOR
tipo: prd
atualizado_em: 2026-08-16
---

# Plano de Implementação — App Desktop "TXTMelhorator"

**Pivô (15/Ago/2026):** de CLI local para **aplicativo desktop** multiplataforma (macOS / Linux / Windows), reaproveitando o pipeline já desenvolvido e testado (extração, limpeza, estrutura, revisão LT).

**Status (16/Ago):** F0 + F1 + fatia F2 **entregues** (core Rust, OCR, janela com drop/preview/export). Este arquivo é o plano original. Estado vivo: [`PRD-MELHORADOR.md`](PRD-MELHORADOR.md) · [`BACKLOG-MELHORADOR.md`](BACKLOG-MELHORADOR.md) · [`HANDOVER-2026-08-16.md`](HANDOVER-2026-08-16.md).

---

## 1. O que o app faz (escopo do produto)

| # | Capacidade | Origem |
|---|---|---|
| C1 | Importar um PDF ou varrer um diretório de **origem escolhido pelo usuário**; **destino também escolhido pelo usuário** (o app pergunta; lembra as últimas escolhas). As pastas `_originais/`/`_output/` são convenção só do CLI de dev — o app não as conhece | novo (UI sobre a lógica do `batch_extract`) |
| C2 | Extrair texto (nativo/OCR) e limpar/estruturar | **reaproveita** `extraction`/`cleanup`/`structure` |
| C3 | Fila de processamento com progresso por livro/etapa e cancelamento | novo |
| C4 | Menu de configuração | novo |
| C4a | — baixar/gerenciar **modelos de IA locais** para revisão opcional do texto | novo (muda a regra zero-IA — ver §5) |
| C4b | — **login LanguageTool** (credenciais Premium) ou servidor local gerenciado | evolução de `languagetool_local` |
| C5 | Revisão com diff aceitar/rejeitar (nada aplicado sem aprovação) | evolução de `lt-local-changes.diff` |
| C6 | Exportar **.md**, **.txt** e **.docx** | novo (md já existe; txt/docx a criar) |

Fora de escopo do app v1: EPUB (ZBOOKER), edição de texto rica, OCR em nuvem, mobile.

## 2. Decisão de stack

**Escolha do Zander (15/Ago):** casca **Tauri 2** (opção A) + requisitos adicionais: **app 100% compilado e sem dependências externas** (o usuário final não instala nada — sem brew, sem Python, sem Java).

Esses requisitos definem a linguagem do core. Comparativo das linguagens candidatas para o *core* (a UI é TypeScript/React de qualquer forma, é o padrão do Tauri):

| Critério | **Rust (no backend do Tauri)** ⭐ | TypeScript (sidecar Bun) | Go (sidecar) | Python (sidecar) |
|---|---|---|---|---|
| "100% compilado" | ✅ nativo, binário único | parcial (JS embutido em executável) | ✅ binário único | ❌ runtime empacotado |
| Sem sidecar (menos peças, 1 assinatura) | ✅ core roda dentro do app | ❌ precisa sidecar | ❌ precisa sidecar | ❌ precisa sidecar |
| Tesseract/PDFium/llama.cpp linkados estaticamente | ✅ crates maduros (leptess, pdfium-render, llama-cpp-2) | ❌ (WASM lento ou binário subprocess) | ⚠️ via cgo (fricção de build 3 SOs) | ⚠️ wheels |
| Port fiel das heurísticas (regex com look-behind/ahead) | ⚠️ via `fancy-regex` | ✅ regex JS suporta nativo | ❌ **RE2 não suporta look-around** — obrigaria reescrever as regras | — (já é o original) |
| Familiaridade sua | média (MegaSena/Tauri, TeclaZ) | alta | baixa | alta |

**Recomendação: Tauri 2 com UI em TypeScript/React e core em Rust.**
- É a única combinação que entrega o requisito por inteiro: um único artefato compilado por SO, sem subprocessos, sem runtimes, uma única assinatura/notarização.
- **Go fica descartado** por um motivo objetivo: o motor de regex do Go (RE2) não tem look-behind/look-ahead, que as regras de limpeza usam intensamente — o port viraria reescrita das heurísticas validadas, o maior risco do projeto.
- **TypeScript fica na UI** (onde brilha); como core, quebraria o "100% compilado" (OCR viraria WASM lento ou subprocess).
- **Python sai do runtime** e vira **implementação de referência**: o port Rust é validado por golden-master — byte a byte contra as saídas do pipeline Python nos 4 livros já processados em `_output/`.

## 3. Arquitetura alvo (independente da stack)

```
┌───────────────────────── App Desktop ─────────────────────────┐
│  UI: origem (PDF/pasta) · fila+progresso · diff · config      │
├───────────────────────────────────────────────────────────────┤
│  core (pacote txtmelhorator — REUTILIZADO)                │
│  extraction → cleanup → structure → report (determinístico)   │
├──────────────────────┬────────────────────┬───────────────────┤
│ Revisão LT           │ Revisão IA (opt-in)│ Exportadores      │
│ · servidor local     │ · modelos GGUF     │ · .md (nativo)    │
│   gerenciado p/ app  │   baixados p/ app  │ · .txt (strip)    │
│ · conta Premium      │ · llama.cpp local  │ · .docx           │
│   (username+apiKey)  │ · SÓ propõe diff   │   (python-docx)   │
└──────────────────────┴────────────────────┴───────────────────┘
```

Princípios preservados: pipeline central **determinístico**; revisão (LT ou IA) **aplica** correções com **Desfazer** e trilha de auditoria (`report.json` / diffs).

**Arquivos e pastas no app:** nada de caminhos fixos. Origem (PDF/pasta) e destino são perguntados ao usuário (diálogos nativos do SO) e lembrados como preferência. Artefatos intermediários (temp de OCR, cache de modelos, `tessdata`) vivem no diretório de dados do app por SO (`~/Library/Application Support/…`, `%APPDATA%`, `~/.local/share/…`) — invisíveis ao usuário e limpáveis nas configurações.

## 4. Portabilidade da extração (maior risco técnico)

Hoje o OCR depende de binários externos instalados via Homebrew: `ocrmypdf` + Ghostscript + qpdf + unpaper. Isso **não é distribuível** em Windows/Linux (e Ghostscript é AGPL — problema de licença para bundle).

**Plano:** trocar a cadeia por bibliotecas **linkadas estaticamente no binário Rust** (licenças permissivas, nada de AGPL):

| Hoje (macOS/brew) | App (linkado no binário, 3 SOs) |
|---|---|
| ocrmypdf + Ghostscript (render) | **PDFium** via `pdfium-render` (BSD) |
| Tesseract via ocrmypdf | **Tesseract + Leptonica** via `leptess`/`tesseract-sys` (Apache 2.0); `tessdata` por+eng embutido ou baixado no 1º uso |
| unpaper (limpeza de imagem) | pré-processamento determinístico próprio em Rust (grayscale, binarização, deskew via crate `image`) |
| qpdf / pypdf | recorte de páginas via PDFium |

`cleanup`/`structure` são **portados para Rust** (`fancy-regex` para look-around) e validados por golden-master contra as saídas Python dos 4 livros. O CLI Python permanece no repo como referência de validação — não é distribuído.

## 5. Revisão de texto no app (menu de configuração)

### LanguageTool — restrição do "sem dependências externas"
O LanguageTool é Java e **não compila** para binário nativo. Com o requisito de app 100% compilado:

- **Conta LanguageTool (nuvem):** caminho principal no app — campos para `username` + `apiKey` da Proofreading API (Premium). Aviso claro: texto vai para a nuvem. Credenciais no keychain do SO — nunca em arquivo texto.
- **Servidor LT local:** vira opção *avançada opcional*: o app detecta um servidor rodando (URL configurável) e usa; **não** embute Java. Quem quiser 100% offline com LT instala por conta própria (ou usa o CLI dev).
- **Revisão offline de primeira classe no app = modelos de IA locais** (llama.cpp compilado no binário — ver abaixo), que cobrem o requisito sem runtime externo.
- Correção herdada do CLI (bug 15/Ago): chunk ≤ 20k para servidor local — 50k derruba o Java com heap padrão.
- Free API pública: **não** (rate-limit e envio à nuvem sem benefício Premium).

### Modelos de IA locais (opt-in) — **mudança formal da regra zero-IA**
O AGENTS.md proíbe IA no pipeline. O pivô **emenda a regra** assim (a ser refletido no AGENTS.md quando o plano for aprovado):

> O pipeline de extração/limpeza/estrutura permanece 100% determinístico e zero-IA. IA local é permitida **exclusivamente** como camada opcional de **revisão**, desligada por padrão, que apenas **propõe** correções via diff; nenhuma saída de modelo substitui texto sem aprovação humana; o diff aprovado fica registrado.

- **Não há servidor de IA:** a inferência roda **in-process** (llama.cpp linkado no binário Rust via `llama-cpp-2`), CPU por padrão, Metal/CUDA quando disponível. Modelo carregado sob demanda e descarregado após a revisão. Nada de Ollama/servidor externo.
- Gerenciador de modelos: download com hash conferido, remoção, seleção do ativo. (Modelos são **dados** baixados on-demand, não dependências: o app funciona sem eles.)

**Catálogo curado inicial (GGUF, revisão gramatical pt-BR):**

| Modelo | Tamanho (Q4) | RAM aprox. | Licença | Papel |
|---|---|---|---|---|
| **Gemma 3 4B instruct (QAT)** | ~2,5 GB | 6 GB | Gemma Terms (uso ok; conferir p/ produto comercial) | padrão recomendado — bom pt-BR, roda em qualquer Mac M1+ |
| **Qwen 3 4B instruct** | ~2,4 GB | 6 GB | Apache 2.0 | alternativa de licença mais limpa |
| **Gemma 3 12B instruct** | ~7 GB | 12 GB | Gemma Terms | tier "qualidade" p/ máquinas fortes |

Critérios de entrada no catálogo: qualidade em pt-BR, licença compatível, GGUF oficial disponível, e **avaliação no nosso benchmark de fidelidade** (corpus de trechos dos livros com erros conhecidos de OCR/gramática: o modelo precisa corrigir sem reescrever estilo nem inventar conteúdo — modelo que "melhora" o autor é reprovado, por melhor que seja a gramática). O download vem do Hugging Face com hash pinado; nada de modelo embutido no instalador.
- Prompt fixo de revisão gramatical com instrução de fidelidade (não reescrever estilo, não completar conteúdo); saída convertida em diff por trecho.
- CoTypist: descartado como integração (app de previsão de digitação, sem API).

## 6. Exportação

| Formato | Como | Observação |
|---|---|---|
| `.md` | já é a saída nativa (`cleaned.md` ou versão com diffs aprovados) | zero trabalho |
| `.txt` | strip determinístico de marcação (headings → linha em caixa própria, listas → travessão) | regras simples, testáveis |
| `.docx` | `python-docx`: H1–H4 → estilos Heading, prosa → Normal, sumário → lista | estilos nomeados p/ o fluxo editorial |

Exportação sempre a partir do texto **aprovado**, com hash do fonte no rodapé de metadados do arquivo (auditabilidade).

## 7. Fases de implementação

| Fase | Entrega | Depende de |
|---|---|---|
| **F0 — Golden masters** | congelar saídas de referência do pipeline Python (4 livros + casos sintéticos dos 61 testes); fix do chunk LT local no CLI | — |
| **F1 — Core Rust** | port de `cleanup`/`structure` para crate Rust (`fancy-regex`), validado byte a byte contra os golden masters; extração PDFium+Tesseract estáticos com paridade de `report.json` | F0 |
| **F2 — MVP app (macOS)** | Tauri 2 + React: importar PDF/pasta → fila com progresso/cancelamento → resultado + avisos → exportar .md/.txt | F1 |
| **F3 — Multiplataforma + DOCX** | builds Linux/Windows do binário único; exportação .docx com estilos | F2 |
| **F4 — Revisão integrada** | config conta LT (keychain) + servidor local opcional; viewer de diff aceitar/rejeitar por ocorrência | F2 |
| **F5 — IA opcional** | emenda zero-IA no AGENTS.md; gerenciador de modelos GGUF; llama.cpp linkado; revisão IA → diff; guardrails | F4 |
| **F6 — Distribuição** | assinatura + notarização macOS; instalador Windows; AppImage/deb; auto-update simples | F3 |

Gates APAE: aprovação deste plano libera F0+F1; F2/F3/F4/F5 têm gate próprio ao iniciar (mudam escopo/estrutura).

## 8. Riscos

| Risco | Mitigação |
|---|---|
| **Port Rust divergir das heurísticas validadas** | golden-master byte a byte contra as saídas Python (4 livros reais + 61 casos de teste); Python fica no repo como referência viva |
| Cadeia de build estático (Tesseract/PDFium/llama.cpp) em 3 SOs | crates com builds prontos; CI por SO desde F1; é o preço do "sem dependências externas" |
| Peso dos downloads (modelos GGUF 2–5 GB) | on-demand com barra de progresso; app base enxuto |
| Licenças de bundle | evitar Ghostscript (AGPL); PySide6 LGPL ok; Tesseract/pypdfium2 Apache/BSD; conferir licença de cada modelo no catálogo |
| OCR de livros grandes trava a UI | pipeline em processo/thread separado, fila cancelável, checkpoint (já existe no batch) |
| IA inventar texto | camada só-diff + prompt de fidelidade + aprovação humana obrigatória (princípio nº 1 do projeto) |
| Assinatura/notarização em 3 SOs | fase própria (F5); começar cedo com macOS (você já tem cadeia no SmartWrite installer) |
| Duas sessões de IA no repo | trabalhar o app em branch própria `feature/app-desktop` |

## 9. Decisões

1. ✅ **Casca:** Tauri 2 (escolha do Zander, 15/Ago).
2. ✅ **Requisito:** 100% compilado, sem dependências externas (Zander, 15/Ago) — **ainda não cumprido no OCR** (Tesseract via Homebrew).
3. ✅ **Core Rust + UI TypeScript** — código no ar (16/Ago).
4. ✅ Trabalho F0+F1 (e fatia F2) executado; o carimbo “aguardando APAE / nenhum código” está superado.
5. ⬜ Nome do app (working title: "TXTMelhorator").
