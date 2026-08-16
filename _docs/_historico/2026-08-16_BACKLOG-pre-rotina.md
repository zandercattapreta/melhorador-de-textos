---
sistema: PROJETO
tipo: backlog
atualizado_em: 2026-08-15
---

# Backlog (arquivo · 15/Ago/2026)

Rotacionado em 16/Ago/2026. Vigente: [`../BACKLOG-MELHORADOR.md`](../BACKLOG-MELHORADOR.md) (fila R1–R5).

---

Origem: AS_IS + implementação 15/Ago.

## Entregue (15/Ago)

- Comando `batch-extract` (descoberta + metadados + sequencial + checkpoint)
- Extração de metadados (autor/título/ISBN) com testes e heurísticas de confiança
- 45 testes passando (35 pipeline + 10 metadata) — depois: 61
- 4 PDFs (3.5K pgs) validados com 100% sucesso
- Pacotes LanguageTool prontos para revisão manual

## P0 (na época)

1. Versionar documentação (`_docs/` no git)
2. Remoto git — **parcialmente superado:** há `origin` no GitHub; `_docs/` e `_APP/` ainda fora do commit completo

## P1 (na época)

3. LanguageTool API vs manual — hoje: LT local no CLI; Premium manual; app ainda sem LT
4. Contrato worker SADE (opcional)
5. OCR integral — mecanismo `--full` pronto; lote já rodou livros inteiros

## P2 (na época)

6. Faixas não contíguas
7. Dicionário OCR
