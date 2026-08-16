# QA editorial — Mesopotâmia páginas 1–50

**Arquivo:** `_output/mesopotamia/pages-001-050/cleaned.md`  
**Data:** 2026-07-25 (reprocessado com fixes)  
**Flags:** `--drop-leading-pages 3`  
**Métricas:** 4 H1 · 17 H2 (6 Title Case) · 1 correção OCR `IL→II` · 3 páginas iniciais descartadas · 2 scans duplicados removidos · 7 `|` isolados · 35 seções embutidas quebradas · 0 `�`

## Veredito

**Aprovado para PoC + revisão humana.** Front-matter e ruído estrutural tratados. Erros de palavra OCR (`pdem`, `jntroduziu-se`) permanecem — só LanguageTool / olho humano.

---

## Fixes aplicados (pipeline)

| # | Fix | Status |
|---|---|---|
| 1 | Front-matter: `--drop-leading-pages N` | ✅ (N=3 na amostra) |
| 2 | Deduplicação de scans pelo nº de rodapé | ✅ (2 páginas) |
| 3 | Número de página após hífen de quebra | ✅ (padrão; amostra limpa via cópia boa) |
| 4 | Title Case curto → H2 | ✅ (6 subtítulos) |
| 5 | Seção `N.` colada após `.` → quebra de parágrafo | ✅ (35) |
| 6 | `|` isolados + glifos `—<——` + fragmento de borda | ✅ |
| 7 | Heading `IL -` → `II. —` (só em heading) | ✅ |

---

## O que está bem

- Corpo começa em “As Primeiras Idades…” (sem ISBN/créditos).
- Subtítulos: `## O tempo absoluto`, `## A vida técnica`, etc.
- `# II. — OS ANTROPIANOS…` (não mais `IL`).
- Sem `|` isolados nem `—<——` no `cleaned.md`.

---

## Residual (revisão humana / LT)

| Severidade | Item |
|---|---|
| P1 | Typos OCR de palavra: `pdem`, `jntroduziu-se`, `comudidade`, aspas/travessões quebrados |
| P2 | Ortografia PT-PT da fonte (`Egipto`, `colectividades`) — não “corrigir” no pipeline |
| P2 | Sem SUMÁRIO neste PDF (esperado) |

---

## Checklist humano

- [ ] Confirmar H1 `I.` / `II.` contra PDF
- [ ] `prepare-lt` a partir do corpo (já sem front-matter)
- [ ] Anotar typos recorrentes para dicionário/regras futuras (sem IA)

---

## Conclusão

Pipeline **cumpre** os fixes P0/P1 estruturais do QA anterior.  
Publicação ainda exige passagem humana + LanguageTool.
