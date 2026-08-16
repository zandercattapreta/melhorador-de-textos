# TXTMelhorator — AGENTS.md

## Overview

Extrai texto de PDFs de livros digitalizados e melhora a conversão: formatação e legibilidade **sem inventar conteúdo**.
Duas superfícies: **App desktop** (`_APP/`, Tauri 2 + core Rust) e **CLI** (`_CLI/`, Python 3.12 — referência + lote). PoC **0.2.0**.

PRD único: [`_docs/PRD-MELHORADOR.md`](_docs/PRD-MELHORADOR.md). Índice: [`_docs/INDEX.md`](_docs/INDEX.md). Estado: [`_docs/arquitetura/AS_IS.md`](_docs/arquitetura/AS_IS.md).

> `.gitignore` ignora `_docs/` — documentação hoje só no disco/tarball.

## Setup / Build / Test

**Estrutura:** `_CLI/` (Python, testes, venv) · `_APP/` (Tauri 2: `core/` Rust + UI React/TS) · `_docs/` · `_originais/` · `_output/` (dados, raiz).

```bash
brew install python@3.12 tesseract tesseract-lang ghostscript qpdf unpaper languagetool
cd _CLI && python3.12 -m venv .venv && source .venv/bin/activate
pip install -e ".[dev]"
python -m pytest                              # 61 testes CLI (de _CLI/)
bash _CLI/melhorar.sh                         # lote CLI (da raiz)
txtmelhorator extract --input "_originais/<arquivo>.pdf" --pages 21-30
cd _APP/core && cargo test --release          # core Rust + goldens (sempre --release)
cd _APP && npm run tauri dev                  # janela do app
```

Gerenciador: `pip` dentro de `_CLI/.venv` (Python 3.12); não misturar com o Python do sistema. Comandos do CLI rodam **da raiz**. App: Rust stable + Node; o CLI é a referência dos golden masters. OCR do app ainda usa Tesseract do Homebrew.

## Code style

Padrão universal: `~docs/~work_guidelines/protocols/CODE_STYLE.md`.

- Comentários inline em PT-BR; identificadores/logs/commits em EN-US.
- Código Debug Ready: lógica complexa, OCR/pipeline e integrações sempre comentados.
- Cabeçalho obrigatório em novos scripts: `~docs/~work_guidelines/templates/SCRIPT_HEADER.md`.

## Testing

- CLI: `cd _CLI && .venv/bin/python -m pytest` (61).
- Core: `cd _APP/core && cargo test --release` (goldens em `_temp/goldens/` — local-only).
- Modo paridade (`clean_text` / `apply_structure`) = CLI. Modo aprimorado = app; não quebrar goldens.
- Teste de máquina ≠ teste de UI. Ver `~docs/~work_guidelines/protocols/DOD.md`.

## Security & Boundaries

**Permitido:** editar código/docs do escopo aprovado, rodar testes/lint quando existirem, criar branch.

**Confirmar (APAE):** deploy, migração estrutural, tocar > 2 arquivos, escolha/troca de stack, **OCR em lote de livros inteiros**.

**Proibido:**
- Commitar ou logar secrets / `.env` / tokens.
- `force push` em `main`/`master` sem autorização explícita.
- Commitar pastas protegidas: `_bkps/ _resources/ _ originais/ _docs/ _tests/`.
- Commitar PDFs brutos ou assets de livros (`_resources/`, `_ originais/`) — local-only.
- Inventar ou “completar” texto do livro que não esteja na fonte OCR/PDF.
- **Usar IA/LLM em extração, OCR, limpeza ou estrutura.** Essas etapas = Tesseract clássico + regras fixas + (no CLI) LanguageTool humano.

**IA local permitida só na revisão (emenda 16/Ago):**
- Opt-in, desligada por padrão, no aparelho (sem nuvem).
- Só **propõe** diff; nada entra no texto sem o Zander aceitar.
- Vocabulário do livro = lista de termos extraídos do próprio OCR/texto nativo, com âncora na fonte — não é invenção.
- Regras que o usuário ensina (marca cabeçalho, nota, etc.) vêm **antes** de qualquer modelo; o modelo só entra se as regras não bastarem.
- Proposta que adicione conteúdo sem âncora no original, ou reescreva o estilo do autor, é rejeitada.
- LanguageTool (local ou Premium) continua válido como revisão sem LLM.

## Commit & PR

- Idioma: EN-US. Conventional Commits (`feat:`, `fix:`, `chore:`).
- Branch: `feature/<slug>`.

## Comunicação

Resultado primeiro. Pouco técnico. Uma decisão por vez. SIM/NÃO ou A/B/C.
SSOT: `~docs/~work_guidelines/protocols/COMMUNICATION.md`.

## Workflows & Skills

| Comando | Ação |
|---|---|
| `/sod` | Start of Day — `.agent/workflows/sod.md` |
| `/eod` | End of Day — `.agent/workflows/eod.md` |
| `/eow` | End of Week — `.agent/workflows/eow.md` |
| `/query` | Consulta read-only — APAE suspenso |
| `/ideacao` | Ideação — `~docs/~work_guidelines/workflows/ideacao.md` |
| `/dev` | Desenvolvimento — `~docs/~work_guidelines/protocols/APAE.md` |
| `/uat` | UAT — `~docs/~work_guidelines/workflows/uat.md` |
| `/bug` | Bug — `~docs/~work_guidelines/workflows/bug.md` |

Skills: `.agent/skills/*/SKILL.md` — ler `golden-rules` primeiro.

## Regras universais (herdadas — não reescrever)

- **Modos:** Ideação / Desenvolvimento / UAT / Bug. `~docs/~work_guidelines/protocols/WORK_MODES.md`
- **APAE:** `~docs/~work_guidelines/protocols/APAE.md`
- **Golden Rules:** `~docs/~work_guidelines/protocols/GOLDEN_RULES.md`
- **Idiomas:** chat/docs PT-BR; código/commits EN-US.
- **Fail-fast:** na 1ª falha de build/deploy/teste, parar e reportar. "PARE" = parada imediata.

## Definition of Done (auto-verificável)

- [ ] Requisitos aprovados atendidos (nada além do escopo).
- [ ] Build/test/lint OK quando a stack existir.
- [ ] Texto melhorado não inventa conteúdo ausente na fonte.
- [ ] Docs atualizados (`_docs/BACKLOG-MELHORADOR.md` / `PRD-MELHORADOR.md`) quando a convenção muda.
- [ ] `bash "../~scripts/docs/check-docs.sh" .` verde (quando tocar `_docs/` no disco).
- [ ] Sem secrets nem PDFs brutos no diff.

DoD universal: `~docs/~work_guidelines/protocols/DOD.md`.

## Dados de referência

| Dado | Valor |
|---|---|
| Caminho do projeto | `/Users/zander/Documents/_ coding/_ TXTMelhorator` |
| Ambiente local | `_CLI/.venv` (Python 3.12) · CLI `txtmelhorator` · Rust 1.97 · Node 26 |
| Código | `_CLI/` (Python, referência) · `_APP/` (Tauri 2: core Rust + UI TS) |
| Saídas | `_output/` e `_temp/` (local-only, fora do git) |
| Produção | N/A (ainda) |

<!-- Última atualização: 2026-08-16 · Versão: 0.2.0-poc -->
