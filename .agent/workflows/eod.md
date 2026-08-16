---
description: End of Day — TXTMelhorator
---

# Workflow: End of Day (EOD)

> **Uso:** ao encerrar cada sessão neste projeto.
> **Tempo estimado:** 5–10 minutos

**Caminho:** `/Users/zander/Documents/_ coding/_ TXTMelhorator`

---

## 1. Verificar Definition of Done

Consultar `.agent/skills/dod/SKILL.md` e `~docs/~work_guidelines/protocols/DOD.md`.

- [ ] Entrega atende o DoD do projeto?
- [ ] Build/testes OK quando a stack existir?

---

## 2. Git — Status e Commit

```bash
cd "/Users/zander/Documents/_ coding/_ TXTMelhorator"
git status
```

Só commitar se o usuário pedir ou autorizar no EOD:

```bash
git add -A
git commit -m "chore(sync): snapshot EOD $(date +%d/%b)" \
  || echo "✅ Nada novo para commitar"
```

```bash
git push origin main \
  || echo "⚠️ Push falhou ou remoto ausente"
```

> Nunca commitar: `_bkps/`, `_resources/`, `_docs/`, `_tests/`, `.env`, secrets, PDFs brutos.

---

## 3. Limpar Artefatos Temporários

```bash
cd "/Users/zander/Documents/_ coding/_ TXTMelhorator"
rm -rf .temp/ _temp/ 2>/dev/null || true
```

---

## 4. Backup Local

```bash
cd "/Users/zander/Documents/_ coding/_ TXTMelhorator"
DATE=$(date +%Y-%m-%d)
zip -r "_bkps/melhorador-de-textos_${DATE}.zip" \
  AGENTS.md README.md .gitignore .agent/ \
  -x "*/.DS_Store" \
  && echo "✅ Backup: _bkps/melhorador-de-textos_${DATE}.zip" \
  || echo "⚠️ Backup falhou"
```

> Incluir `src/` / manifests quando a stack existir. Não zipar PDFs de `_resources/` sem pedido.

---

## 5. Atualizar Backlog

- [ ] `_docs/BACKLOG-MELHORADOR.md` — marcar concluídos; adicionar itens novos.

---

## 6. Atualizar CHANGELOG.md

Entrada do dia em `_docs/CHANGELOG.md`.

---

## 7. Handover

Atualizar ou criar `HANDOVER_*.md` em `_docs/` (perguntar qual se não estiver claro).

Conteúdo mínimo: Estado Atual, P0 próxima sessão, Prompt para o próximo agente, Bloqueadores.

---

## 8. Preparar Próxima Sessão

- [ ] Prioridades em `task.md`
- [ ] Bloqueadores documentados

---

## Checklist Final do EOD

- [ ] DoD verificado?
- [ ] Git status/commit conforme autorização?
- [ ] Backlog e CHANGELOG atualizados?
- [ ] Handover atualizado?
- [ ] Próxima sessão planejada?
