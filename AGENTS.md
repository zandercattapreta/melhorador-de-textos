# Melhorador de Textos — AGENTS.md

## Overview

Extrai texto de PDFs de livros digitalizados e melhora a conversão: corrige formatação e legibilidade sem inventar conteúdo do livro.
Stack: Python 3.12 + OCRmyPDF/Tesseract (`por+eng`) + pypdf + ftfy. Deploy: local (CLI). PoC **0.1.0**.

Documentação: [`_docs/INDEX.md`](_docs/INDEX.md). Estado: [`_docs/arquitetura/AS_IS.md`](_docs/arquitetura/AS_IS.md).

> `.gitignore` ignora `_docs/` — documentação hoje só no disco/tarball.

## Setup / Build / Test

```bash
brew install python@3.12 tesseract tesseract-lang ghostscript qpdf unpaper  # deps nativas
python3.12 -m venv .venv && source .venv/bin/activate                       # ambiente
pip install -e ".[dev]"                                                      # instala projeto
python -m pytest                                                            # testes
melhorador-textos extract --input "_ originais/<arquivo>.pdf" --pages 21-30  # extração+limpeza
melhorador-textos prepare-lt --input "_output/<doc>/pages-XXX-YYY/cleaned.md"
melhorador-textos import-lt --original <original.txt> --corrected <corrected.md>
```

Gerenciador: `pip` dentro de `.venv` (Python 3.12). Não misturar com o Python do sistema (3.9).

## Code style

Padrão universal: `~docs/~work_guidelines/protocols/CODE_STYLE.md`.

- Comentários inline em PT-BR; identificadores/logs/commits em EN-US.
- Código Debug Ready: lógica complexa, OCR/pipeline e integrações sempre comentados.
- Cabeçalho obrigatório em novos scripts: `~docs/~work_guidelines/templates/SCRIPT_HEADER.md`.

## Testing

- `python -m pytest` (testes de `cleanup`, `structure` e `languagetool_review` em `tests/`).
- Smoke test de OCR real: `extract --pages 1-50` no PDF Mesopotâmia.
- Teste de máquina ≠ teste de UI. Ver `~docs/~work_guidelines/protocols/DOD.md`.

## Security & Boundaries

**Permitido:** editar código/docs do escopo aprovado, rodar testes/lint quando existirem, criar branch.

**Confirmar (APAE):** deploy, migração estrutural, tocar > 2 arquivos, escolha/troca de stack, **OCR integral de PDFs grandes** (a PoC processa só faixas de páginas; o livro completo de 578 páginas exige autorização).

**Proibido:**
- Commitar ou logar secrets / `.env` / tokens.
- `force push` em `main`/`master` sem autorização explícita.
- Commitar pastas protegidas: `_bkps/ _resources/ _ originais/ _docs/ _tests/`.
- Commitar PDFs brutos ou assets de livros (`_resources/`, `_ originais/`) — local-only.
- Inventar ou “completar” texto do livro que não esteja na fonte OCR/PDF.
- **Usar IA/LLM** em qualquer etapa do pipeline (OCR neural generativo, reescrita por modelo, classificação por embedding, etc.). Stack = OCR clássico + heurísticas determinísticas + revisão humana opcional.

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
- [ ] Build/test/lint OK quando a stack existir; nesta fase scaffold, estrutura e docs consistentes.
- [ ] Texto melhorado não inventa conteúdo ausente na fonte.
- [ ] Docs atualizados (`_docs/BACKLOG.md` / PRD) quando a convenção muda.
- [ ] `bash "../~scripts/docs/check-docs.sh" .` verde (quando tocar `_docs/` no disco).
- [ ] Sem secrets nem PDFs brutos no diff.

DoD universal: `~docs/~work_guidelines/protocols/DOD.md`.

## Dados de referência

| Dado | Valor |
|---|---|
| Caminho do projeto | `/Users/zander/Documents/_ coding/_ melhorador de textos` |
| Ambiente local | `.venv` (Python 3.12) · CLI `melhorador-textos` |
| Saídas | `_output/` e `_temp/` (local-only, fora do git) |
| Produção | N/A (ainda) |

<!-- Última atualização: 2026-07-25 · Versão: 0.1.0-poc -->
