---
description: Start of Day — Melhorador de Textos
---

# Workflow: Start of Day (SOD)

> **Uso:** início de cada sessão neste projeto.
> **Tempo estimado:** 5–10 minutos

**Caminho:** `/Users/zander/Documents/_ coding/_ melhorador de textos`

---

## 1. Verificar Estado do Git

```bash
cd "/Users/zander/Documents/_ coding/_ melhorador de textos"
git status
git log --oneline -5
```

**Verificar:**
- Mudanças não commitadas da sessão anterior?
- Branch `main` sincronizado com o remoto (quando houver remoto)?

Se houver mudanças não commitadas e o usuário autorizar commit:
```bash
git add -A && git commit -m "chore: WIP — início de sessão"
```

---

## 2. Autenticações

N/A até existir integração com serviços externos.

---

## 3. Ambiente local / saúde

N/A até a stack ser definida. Não subir `dev-up-all.sh` só por este projeto.

Quando houver app local, documentar aqui o comando de health-check.

---

## 4. Carregar Contexto do Projeto

Ler na ordem:

1. `AGENTS.md`
2. `.agent/skills/golden-rules/SKILL.md`
3. `_docs/INDEX.md`
4. `_docs/BACKLOG.md` (primeiras ~60 linhas)
5. `_docs/CHANGELOG.md` (opcional, topo)

**Confirmar:**
- Ciclo A.P.A.E. ativo
- Fidelidade à fonte (sem inventar texto do livro)
- Estado do backlog compreendido

---

## 5. Produção

N/A — sem deploy ainda.

---

## 6. Definir Objetivos do Dia

Criar/atualizar `task.md` com prioridades, bloqueadores e contexto.

> **PONTO DE CHECAGEM APAE:** `[A.P.A.E] Posso prosseguir?`
> Aguardar "Sim" antes de escrever código.

---

## Checklist Final do SOD

- [ ] Git status verificado?
- [ ] `AGENTS.md`, golden-rules, CHANGELOG e BACKLOG lidos?
- [ ] Objetivos do dia em `task.md`?
- [ ] Autorização obtida para iniciar?
