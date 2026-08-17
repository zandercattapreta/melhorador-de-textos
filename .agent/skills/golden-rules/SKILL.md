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
5. **Revisão IA/LT (opt-in):** no aparelho (ou nuvem com aviso); **aplica** correções; **Desfazer** restaura. Vocabulário do livro = termos da própria fonte. Regras do usuário vêm antes do modelo. Sem inventar conteúdo nem reescrever o autor.
6. **Skills primeiro:** ler esta skill e o `AGENTS.md` antes de alterar código ou docs de processo.
