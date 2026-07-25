---
description: End of Week — Melhorador de Textos
---

# Workflow: End of Week (EOW)

> **Uso:** fim de cada semana (sexta ou último dia útil).
> **Tempo estimado:** 15–20 minutos

**Caminho:** `/Users/zander/Documents/_ coding/_ melhorador de textos`

---

## Pré-requisito: EOD primeiro

Executar `.agent/workflows/eod.md` antes do EOW. Sincronizar estados de chats da semana (Git/docs).

---

## 1. Testes E2E / produção

N/A até existir stack e deploy. Quando houver:

```bash
# documentar health-check aqui
```

Fail-fast: se serviço crítico falhar, parar e notificar — sem debug sozinho em produção.

---

## 2. Backup Semanal (Local)

```bash
cd "/Users/zander/Documents/_ coding/_ melhorador de textos"
SEMANA=$(date +%Y-W%V)
zip -r "_bkps/melhorador-de-textos_${SEMANA}.zip" \
  AGENTS.md README.md .gitignore .agent/ \
  -x "*/.DS_Store" \
  && echo "✅ Backup semanal: _bkps/melhorador-de-textos_${SEMANA}.zip" \
  && ls -lh "_bkps/melhorador-de-textos_${SEMANA}.zip" \
  || echo "⚠️ Backup falhou"
```

---

## 3. Auditoria de Documentação

```bash
cd "/Users/zander/Documents/_ coding/_ melhorador de textos"
echo "=== Documentação Core ==="
[ -f "AGENTS.md" ]              && echo "✅ AGENTS.md"              || echo "❌ AGENTS.md AUSENTE"
[ -f "_docs/CHANGELOG.md" ]     && echo "✅ CHANGELOG.md"           || echo "❌ CHANGELOG.md AUSENTE"
[ -f "_docs/BACKLOG.md" ]       && echo "✅ BACKLOG.md"             || echo "❌ BACKLOG.md AUSENTE"
[ -f "README.md" ]              && echo "✅ README.md"              || echo "❌ README.md AUSENTE"
[ -f ".agent/workflows/sod.md" ] && echo "✅ sod.md"                || echo "❌ sod.md AUSENTE"
```

**Revisão:**
- [ ] Backlog reflete o estado real?
- [ ] CHANGELOG cobre a semana?
- [ ] Handovers dos módulos trabalhados atualizados?

---

## 4. Git — Fechamento de Semana

```bash
cd "/Users/zander/Documents/_ coding/_ melhorador de textos"
git add -A && git status --short
```

Só commitar/push se autorizado:

```bash
git commit -m "chore(eow): fechamento da semana $(date +%Y-W%V)" \
  || echo "✅ Commit já realizado pelo EOD"
git push origin main || echo "⚠️ Remoto ausente ou push falhou"
```

---

## 5. Handover de Fechamento

Incluir: decisões da semana, saúde operacional (N/A por enquanto), pendências P1/P2/P3.

---

## 6. Planejamento da Próxima Semana

- [ ] Prioridades (ex.: stack, pipeline PDF)
- [ ] Deploys / expirações (quando aplicável)

---

## Checklist Final do EOW

- [ ] EOD executado?
- [ ] Backup semanal gerado?
- [ ] Documentação auditada?
- [ ] Handover P1/P2/P3 atualizado?
