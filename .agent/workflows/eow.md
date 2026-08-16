---
description: End of Week — TXTMelhorator
---

# Workflow: End of Week (EOW)

> **Uso:** fim de cada semana (sexta ou último dia útil).
> **Tempo estimado:** 15–20 minutos

**Caminho:** `/Users/zander/Documents/_ coding/_ TXTMelhorator`

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
cd "/Users/zander/Documents/_ coding/_ TXTMelhorator"
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
cd "/Users/zander/Documents/_ coding/_ TXTMelhorator"
bash "../~scripts/docs/check-docs.sh" .
```

**Revisão:**
- [ ] `_docs/BACKLOG-MELHORADOR.md` reflete o estado real?
- [ ] `_docs/CHANGELOG.md` cobre a semana?
- [ ] `python -m pytest` verde se houve mudança de código?

---

## 4. Git — Fechamento de Semana

```bash
cd "/Users/zander/Documents/_ coding/_ TXTMelhorator"
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
