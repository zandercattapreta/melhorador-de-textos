---
sistema: MELHORADOR
tipo: backlog
atualizado_em: 2026-08-16
---

# Backlog — TXTMelhorator

Fonte de produto: [`PRD-MELHORADOR.md`](PRD-MELHORADOR.md) §5 · motores: [`ARQUITETURA-MELHORADOR.md`](ARQUITETURA-MELHORADOR.md) §2 · UI: [`DESIGN-SYSTEM-APP.md`](DESIGN-SYSTEM-APP.md).

Fila de construção do **app** = **R1 → R5** (R5c aberta) · redesign UI = **U8 → U10**. CLI = referência estável (ver § CLI).

---

## Agora (fila do app)

| ID | Entrega | PRD | Estado |
|---|---|---|---|
| **R1** | Pasta + idioma + fila + salvar no fim | A6, A7, A8 | ✅ |
| **R2** | Layout: colunas, juntas, blocos, tabelas, notas | A13, M2–M6 | ✅ (+ preprocess contraste; hOCR fino = depois) |
| **R3** | Conferência lado a lado | A15, M10 | ✅ |
| **R4** | Regras que o usuário ensina | A14, M7 | ✅ |
| **R5** | IA local opt-in + vocabulário | A16, M8–M9 | ✅ in-process (`llama-cpp-2` + Metal) |

### Detalhe R1–R4
- [x] R1a+R1b, R2a+R2b1+R2b2, R3a+R3b, R4

### Detalhe R2 (resto)
- [x] Pré-processamento imagem (contraste stretch antes do OCR)
- [ ] Depois: tabelas via hOCR / posições finas (opcional QA)

### Detalhe R5
- [x] Emenda AGENTS + golden-rules
- [x] Vocabulário + guardrail + diff aceitar/rejeitar
- [x] Benchmark de fidelidade pt-BR (teste `fidelity_benchmark_ok`)
- [x] Gerenciador GGUF (download+hash, remoção, seleção; CoTypist Gemma)
- [x] UI: botão IA local / nuvem / LT (aplica + Desfazer; sem checklist de sugestões)
- [x] **R5c — Inferência embutida:** `llama.cpp` linkado no binário (`llama-cpp-2` + Metal); **sem** `llama-cli`/Ollama/app externo
- [x] Remover dependência de binário no PATH
- [~] PoC antigo via `llama-cli` no PATH — **removido** (16/Ago)

### Revisão LT no app
- [x] Conta Premium no keychain + aviso de nuvem (A10)
- [x] Servidor LT local por URL (sem Java no bundle)
- [x] Trilha: `*.report.json` com hash + diffs aprovados na exportação

### Export e distribuição
- [x] `.docx` com estilos (A9)
- [x] Tessdata no bundle (script `bundle-tessdata.sh` + resource_dir; treinado local-only)
- [x] Scripts assinatura/notarização macOS (`notarize-macos.sh`) — **fora:** Zander não terá Apple Developer pago; distribuição = GitHub Releases + “clique direito → Abrir”
- [ ] Builds Linux/Windows + auto-update (E6) — GitHub Releases (sem notarização Apple)

### Ops
- [x] Commitar working tree (`feature/backlog-r1-r5-close`)
- [x] Versionar `_docs/` (removido do `.gitignore`)
- [x] Port `metadata` (ficha → slug) no core Rust
- [x] `report.json` no app (ao salvar)

### UI — redesign (Claude / frontend-design + DS)

Pedido Zander 16/Ago: redesenhar a interface via skill Claude Design / `frontend-design`, alinhado a [`DESIGN-SYSTEM-APP.md`](DESIGN-SYSTEM-APP.md) (papel e tinta; texto do livro = protagonista).

| ID | Entrega | Estado |
|---|---|---|
| **U1** | Layout B (16/Ago): barra única + 2 colunas Original \| Texto; drawer Ajustes/Revisão | ✅ |
| **U1b** | Wizard config: 1º run + pergunta antes de cada **transcrição/scan** (colunas, ilustrações, idioma, LT/IA) | ✅ |
| **U1c** | Revisão em paralelo com OCR (texto já revisado na caixa) | ❌ **quebrado** — trava (reload GGUF/página); Zander EOD 16/Ago |
| **U2** | Tipografia Source Serif 4 no texto do livro; rail quieto | ✅ |
| **U3** | Revisão / Ajustes / modelos / LT em painéis recolhidos (não entulham) | ✅ |
| **U4** | Remover slogans e ruído de UI | ✅ |
| **U5** | PDF acompanha OCR (preview no evento de progresso; sem reabrir PDF) | ✅ |
| **U6** | OCR híbrido (nativo + OCR só em páginas mudas/capa) | ✅ |
| **U7** | Conferência pós-processamento: Ant/Próx sync página↔texto | ✅ (já R3; manter no redesign) |
| **U8** | Polish DS: tema claro/escuro, densidade, foco teclado, empty states | ⬜ |
| **U9** | Tela fila/biblioteca (T1 do DS) se ainda fizer sentido vs. fluxo atual | ⬜ |
| **U10** | Diff viewer de revisão (inline serif) conforme DS §2 | ⬜ |

**Critério de pronto U\*:** Zander usa 1 livro completo sem “Carregando…” eterno; 3 colunas legíveis; revisão só sob demanda.

### IA local — runtime no app (produto)

| ID | Entrega | Estado |
|---|---|---|
| **R5c** | `llama.cpp` **in-process** no app (sem app externo) | ✅ |
| **R5d** | Carregar GGUF sob demanda / descarregar após revisão | ✅ (load por chamada) |
| **R5e** | Metal (Apple Silicon) quando disponível | ✅ (feature metal) |

Alinhado a [`PLANO-APP-MELHORADOR.md`](PLANO-APP-MELHORADOR.md) e [`ARQUITETURA-MELHORADOR.md`](ARQUITETURA-MELHORADOR.md): “Não há servidor de IA… llama.cpp linkado no binário”.

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
- [x] Redesign UI U1–U7 (em curso U8–U10)

---

## Arquivo

Épicos E0–E6 absorvidos. Histórico: [`PLANO-APP-MELHORADOR.md`](PLANO-APP-MELHORADOR.md).
