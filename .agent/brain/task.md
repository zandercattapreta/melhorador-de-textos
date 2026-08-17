# Task — P0 revisão ao vivo (handover)

Estado: **bloqueado / falhou UAT**. Zander passou a outra IA.

## Pedido (não diluir)

Captura OCR **e** revisão (LT/IA) **ao mesmo tempo**. Caixa de texto = texto já melhorado. Sem travar. Sem checklist.

## Causa raiz (evidência)

- `propose_review` sync + `llama_infer::generate` recarrega ~6 GB **por página**
- Log: **17 recargas** GGUF numa sessão
- Zander **NÃO** autorizou “IA só no fim” como solução

## Próximo passo

Desenhar runtime: modelo **uma vez**, comando **async**, fila; UAT antes de mais features.

Ver: `_docs/HANDOVER-2026-08-16-EOD.md`
