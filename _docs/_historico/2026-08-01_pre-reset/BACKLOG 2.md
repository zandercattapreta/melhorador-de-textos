# Backlog — Melhorador de Textos

## Em Progresso
- (nenhum)

## Próximos
- [ ] Empacotar como worker/API consumível pelo SADE (contrato I/O; ver `docs/INTEGRACAO_SADE.md`)
- [ ] Rodar OCR do livro completo (578 páginas) — requer autorização APAE
- [ ] Integração automática com a Proofreading API do LanguageTool (quando houver `username` + `apiKey`)
- [ ] Suporte a faixas de páginas não contíguas
- [ ] Validar SUMÁRIO quando o PDF tiver índice OCR-ável
- [ ] Dicionário/regras de typos OCR recorrentes (sem IA; pós-revisão LT)

## Concluído
- ✅ Estrutura de pastas e `.agent/` (skills + workflows)
- ✅ `AGENTS.md` + adapters CLAUDE/GEMINI
- ✅ PoC: extração (nativa/OCR) + limpeza determinística + CLI
- ✅ Remoção de cabeçalhos/rodapés e números de página
- ✅ Detecção H1–H4 + SUMÁRIO (`structure.py`, sem IA)
- ✅ Amostra expandida: páginas 1–50 (`_output/mesopotamia/pages-001-050/`)
- ✅ Fixes pós-QA 1–50: front-matter, dedupe de scan, Title Case H2, seções embutidas, glifos OCR, `IL→II`
- ✅ Fluxo manual auditável do LanguageTool
- ✅ Testes unitários + smoke OCR
- ✅ Documentação SADE + regra zero-IA
- ✅ Mapa de ferramentas editoriais para ZBOOKER (índices, citações) — SSOT no SADE + ponteiro local
