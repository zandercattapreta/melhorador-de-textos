# UAT Report — Melhorador de Textos

**Modo:** UAT — **ENCERRADO**  
**Início:** 2026-08-16  
**Fim:** 2026-08-16  
**Branch:** `feature/backlog-r1-r5-close`  
**Veredito Zander:** app inútil por enquanto

## Sob teste

App desktop: processar PDF → revisão → aplicar → salvar · conferência página|texto

## Passou

_(nenhum registrado)_

## Falhou (bugs / atritos) — lista final

| ID | Falha |
|---|---|
| **F1** | Sem botão **Descobrir** (ex. LanguageTool deve falar com o app instalado e puxar infos) |
| **F2** | Download de modelo por URL vazia — usuário não sabe URL; precisa lista/descoberta (estilo Cursor/CoTypist) |
| **F3** | Layout errado — alvo: COL1 setup+PDF · COL2 visualizador · COL3 texto revisado |
| **F4** | Mensagem IA local incompreensível (`heuristic+unavailable_llm`, brew/llama-cli) — sem valor claro para o usuário |
| **F5** | **Próx** atualiza o PDF, coluna de texto não captura (só `[figura]`) — app inútil na conferência |

## Encerramento

UAT fechado pelo Zander (“fim UAT”). Próximo: Desenvolvimento + plano só com Fails.
