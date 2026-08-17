---
sistema: TXTMELHORATOR
tipo: experimento
atualizado_em: 2026-08-16
---

# Experimento — revisão ao vivo vs travamento

## Log sessão app (316539)

| Métrica | Valor |
|---|---|
| `print_info: file size = 6.23 GiB` | **17** |
| Conclusão | GGUF recarregado ~17× na mesma sessão |

Bench `cargo run --example bench_live_review` foi **abortado** (PARE) — sem timing completo de 1× generate.

## Código

- `propose_review`: sync
- `generate`: load → infer → free por chamada
- UI: `propose_review` por página durante OCR

## Pedido vs implementação

Pedido = paralelismo captura+revisão sem freeze.  
Implementação atual = incompatível com o motor pontual.
