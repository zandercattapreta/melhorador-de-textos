---
sistema: MELHORADOR
tipo: backlog
atualizado_em: 2026-08-16
---

# Backlog — Melhorador de Textos

Fonte de produto: [`PRD-MELHORADOR.md`](PRD-MELHORADOR.md) §5 · motores: [`ARQUITETURA-MELHORADOR.md`](ARQUITETURA-MELHORADOR.md) §2.

Fila de construção do **app** = **R1 → R5**. CLI = referência estável (ver § CLI).

---

## Agora (fila do app)

| ID | Entrega | PRD | Estado |
|---|---|---|---|
| **R1** | Pasta + idioma + fila + salvar no fim | A6, A7, A8 | ✅ |
| **R2** | Layout: colunas, juntas, blocos, tabelas, notas | A13, M2–M6 | ✅ (+ preprocess contraste; hOCR fino = depois) |
| **R3** | Conferência lado a lado | A15, M10 | ✅ |
| **R4** | Regras que o usuário ensina | A14, M7 | ✅ |
| **R5** | IA local opt-in + vocabulário | A16, M8–M9 | ✅ (heurística + GGUF via llama-cli + UI) |

### Detalhe R1–R4
- [x] R1a+R1b, R2a+R2b1+R2b2, R3a+R3b, R4

### Detalhe R2 (resto)
- [x] Pré-processamento imagem (contraste stretch antes do OCR)
- [ ] Depois: tabelas via hOCR / posições finas (opcional QA)

### Detalhe R5
- [x] Emenda AGENTS + golden-rules
- [x] Vocabulário + guardrail + diff aceitar/rejeitar
- [x] Benchmark de fidelidade pt-BR (teste `fidelity_benchmark_ok`)
- [x] Gerenciador GGUF (download+hash, remoção, seleção)
- [x] `llama.cpp` via binário no PATH (`llama-cli`); prompt de fidelidade → diff

### Revisão LT no app
- [x] Conta Premium no keychain + aviso de nuvem (A10)
- [x] Servidor LT local por URL (sem Java no bundle)
- [x] Trilha: `*.report.json` com hash + diffs aprovados na exportação

### Export e distribuição
- [x] `.docx` com estilos (A9)
- [x] Tessdata no bundle (script `bundle-tessdata.sh` + resource_dir; treinado local-only)
- [x] Scripts assinatura/notarização macOS (`notarize-macos.sh`) — **execução** exige Apple ID do Zander
- [ ] Builds Linux/Windows + auto-update (E6) — requer CI/loja; fora desta máquina

### Ops
- [ ] Commitar working tree (autorizado neste ciclo — fazer após verde)
- [x] Versionar `_docs/` (removido do `.gitignore`)
- [x] Port `metadata` (ficha → slug) no core Rust
- [x] `report.json` no app (ao salvar)

---

## Feito (resumo)

### CLI
- [x] Pipeline + batch + LT + 61+ testes
- [x] Faixas não contíguas (`1-10,50-60`)
- [x] Dicionário OCR typos (`ocr_typos.py`)
- [ ] Metadados em PDF só-OCR no CLI (core Rust já cobre ficha→slug; CLI nativo pypdf permanece)
- [ ] Contrato worker SADE — **bloqueado**: decisão de produto pendente

### App
- [x] R1–R5 + LT + docx + report + GGUF manager + metadata + preprocess

---

## Arquivo

Épicos E0–E6 absorvidos. Histórico: [`PLANO-APP-MELHORADOR.md`](PLANO-APP-MELHORADOR.md).
