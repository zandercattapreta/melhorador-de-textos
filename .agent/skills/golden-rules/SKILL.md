---
name: Golden Rules
description: Regras fundamentais do projeto Melhorador de Textos
---

# Skill: Golden Rules — Melhorador de Textos

Consulte as regras universais em:
`~docs/~work_guidelines/protocols/GOLDEN_RULES.md`

## Quando usar

- Início de sessão (`/sod`)
- Dúvida de procedimento ou boundary
- Após qualquer violação de regra

## Regras específicas deste projeto

1. **Fidelidade à fonte:** melhorar formatação/legibilidade; nunca inventar, completar ou “corrigir o sentido” do texto do livro além do que a fonte OCR/PDF permite inferir com segurança.
2. **PDFs e assets:** PDFs brutos e materiais de livro ficam em `_resources/` ou `_ originais/` (local-only) — nunca commitar.
3. **Pipeline em lote:** OCR/processamento em massa de PDFs grandes exige autorização APAE (“Sim”).
4. **Zero IA:** não usar LLM, embeddings nem OCR neural generativo nesta ferramenta (economia de tokens e previsibilidade). Só Tesseract/OCRmyPDF, heurísticas e revisão humana opcional (LanguageTool).
5. **Skills primeiro:** ler esta skill e o `AGENTS.md` antes de alterar código ou docs de processo.
