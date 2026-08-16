---
name: Golden Rules
description: Regras fundamentais do projeto TXTMelhorator
---

# Skill: Golden Rules — TXTMelhorator

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
4. **Pipeline zero-IA:** extração, OCR, limpeza e estrutura sem LLM, embeddings ou OCR neural generativo. Só Tesseract/OCRmyPDF e regras.
5. **Revisão IA (opt-in):** permitida só no aparelho, desligada por padrão, só propõe diff. Vocabulário do livro = termos da própria fonte. Regras que o usuário ensina vêm antes do modelo. Nada entra no texto sem aprovação humana.
6. **Skills primeiro:** ler esta skill e o `AGENTS.md` antes de alterar código ou docs de processo.
