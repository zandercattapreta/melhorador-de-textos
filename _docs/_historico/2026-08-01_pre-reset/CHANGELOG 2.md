# Changelog

## [Não Lançado]

### Adicionado
- PoC de pipeline OCR: CLI `txtmelhorator` (Python 3.12) com comandos `extract`, `prepare-lt`, `import-lt`.
- Extração nativa (pypdf) com fallback OCR (OCRmyPDF + Tesseract `por+eng`, `--clean`).
- Limpeza determinística (`ftfy` + heurísticas): Unicode NFC, caracteres invisíveis, marcadores de página, de-hifenização de quebra de linha, reflow de parágrafos.
- Fluxo manual auditável do LanguageTool Premium: pacote com hash SHA-256, manifesto e diff unificado (sem aplicar sugestões automaticamente).
- Testes (`pytest`) de `cleanup` e `languagetool_review`; smoke test OCR nas páginas 21–30 do livro Mesopotâmia (27k chars, 76 hifenizações unidas, 0 caracteres corrompidos).
- `pyproject.toml`, `src/txtmelhorator/`, `tests/`; `.gitignore` protegendo `_output/`.

### Adicionado
- Documentação de integração SADE: `docs/INTEGRACAO_SADE.md` (pipeline, ferramentas, contrato sugerido).
- Regra **zero-IA** no `AGENTS.md` e na skill `golden-rules`.
- Estruturação Markdown determinística (`structure.py`): H1–H4, SUMÁRIO/ÍNDICE; filtro de colofão.
- Amostra OCR expandida: páginas **1–50** do Mesopotâmia.
- Mapa ZBOOKER de ferramentas editoriais (índices, citações): SSOT em `_ zedicoes-sade/.../ZBOOKER_FERRAMENTAS_EDITORIAIS.md` + ponteiro `docs/ZBOOKER_FERRAMENTAS.md`.

### Alterado
- Limpeza: remoção de cabeçalhos correntes, números de página e ruído de borda (por `\f` do OCR).
- CLI `extract` aplica `apply_structure` após cleanup e registra `structure_stats` no relatório.
- Limpeza pós-QA: `--drop-leading-pages`, dedupe de scans duplicados, `|` isolados, glifos `—<——`, nº de página após hífen, quebra de seções `N.` coladas.
- Estrutura: subtítulos Title Case curtos → H2; correção OCR `IL → II. —` só em headings.
