---
sistema: MELHORADOR
tipo: arquitetura
atualizado_em: 2026-08-15
---

# Design System + Interface — App Melhorador de Textos

DS do aplicativo desktop (Tauri 2 + React/TypeScript). Complementa o [`PLANO-APP-MELHORADOR.md`](PLANO-APP-MELHORADOR.md).
Personalidade: **ferramenta editorial** — sóbria, densa em informação, tipografia em primeiro plano, zero enfeite. O texto do livro é o protagonista; a UI é moldura.

---

## 1. Fundações (design tokens)

Implementação: CSS custom properties + Tailwind v4; componentes base shadcn/ui; ícones Lucide. Tokens são a fonte da verdade — nenhum componente usa cor/tamanho literal.

### 1.1 Cor

Tema claro e escuro desde o dia 1 (segue o SO; override manual nas configurações).

| Token | Claro | Escuro | Uso |
|---|---|---|---|
| `--bg` | `#FAFAF8` | `#1A1A1E` | fundo da janela (off-white "papel") |
| `--surface` | `#FFFFFF` | `#232329` | cards, painéis |
| `--surface-2` | `#F1F1ED` | `#2C2C33` | linhas alternadas, wells |
| `--border` | `#E3E3DE` | `#3A3A42` | divisores, contornos |
| `--text` | `#1F1F23` | `#EDEDEA` | texto primário |
| `--text-2` | `#6E6E74` | `#A3A3AA` | secundário, metadados |
| `--accent` | `#8B4A2F` | `#C97B54` | ação primária (terracota — "tinta e papel") |
| `--accent-fg` | `#FFFFFF` | `#1A1A1E` | texto sobre accent |
| `--ok` | `#2E7D4F` | `#5DBB8A` | sucesso, concluído |
| `--warn` | `#B07C1F` | `#E0B45C` | avisos do report |
| `--err` | `#B3372E` | `#E06C60` | erro, falha de OCR |
| `--diff-add` | `#E6F4EA` | `#1E3A2A` | fundo de inserção no diff |
| `--diff-del` | `#FDE8E6` | `#3E2523` | fundo de remoção no diff |

Regra: contraste mínimo AA (4.5:1) em todo texto; estados de erro/aviso sempre com ícone + cor (nunca só cor).

### 1.2 Tipografia

| Papel | Fonte | Tamanho/altura |
|---|---|---|
| UI (labels, menus, botões) | system stack (`-apple-system, Segoe UI, Ubuntu, sans-serif`) | 13/20 base; 11/16 meta; 15/22 títulos de seção |
| **Texto do livro** (preview, diff) | serif embutida — **Source Serif 4** (OFL) | 16/26, largura máx. 68ch |
| Código/hashes/paths | `ui-monospace, SF Mono, Consolas` | 12/18 |

O preview do livro usa serif e medida de leitura confortável — é o "produto" na tela.

### 1.3 Espaço, forma, elevação

- Grade de **4 pt** (4/8/12/16/24/32). Densidade desktop: paddings 8–12 na maioria dos controles.
- Raio: `6px` controles, `10px` cards/modais. Sem sombras fortes: elevação por borda + sombra sutil (`0 1px 3px rgba(0,0,0,.08)`).
- Ícones Lucide 16 px (inline) / 20 px (navegação), stroke 1.75.

### 1.4 Movimento

Funcional e curto: 120–180 ms, ease-out. Barra de progresso e spinner são os únicos elementos animados persistentes. Nada de animação decorativa durante OCR (máquina já está ocupada).

## 2. Componentes

| Componente | Especificação |
|---|---|
| **Botão** | primário (accent, só 1 por tela), secundário (contorno), fantasma (toolbar), destrutivo (err). Altura 32; com ícone opcional à esquerda |
| **Dropzone** | área pontilhada `--border`, ícone livro; aceita arrastar PDF(s)/pasta; hover realça com accent |
| **Card de livro** (fila) | capa-placeholder + título (slug legível) + badge de engine (`OCR`/`nativo`) + barra de progresso por etapa + ações (pausar/cancelar/abrir) |
| **Barra de progresso** | por livro: etapas discretas `extração → limpeza → estrutura → revisão`; cada etapa com check `--ok` ao concluir |
| **Badge de status** | `pendente` (neutro) · `processando` (accent, pulso) · `concluído` (ok) · `aviso` (warn) · `falhou` (err) |
| **Diff viewer** (peça central) | serif, lado único (inline): remoções `--diff-del` tachadas, inserções `--diff-add`; navegação ocorrência-a-ocorrência (`↑↓`), botões Aceitar `⏎` / Rejeitar `⌫` / Aceitar todas da mesma regra; contador "12/87"; painel lateral com a regra LT/modelo que gerou a proposta |
| **Tabela de avisos** | do `report.json`: caracteres ilegíveis (�), páginas duplicadas removidas, hifenizações — clicável → salta para o ponto no preview |
| **Campos de config** | inputs 32px; credenciais com olho + aviso "guardado no chaveiro do sistema"; selects nativos |
| **Card de modelo IA** | nome + tamanho + RAM + licença + estado (baixar % / ativo / remover); botão de download com progresso e hash conferido |
| **Toast** | canto inferior direito, 4s, com ação opcional ("Exportado ✓ — Mostrar no Finder") |
| **Modal de confirmação** | apenas para: livro inteiro (--full), cancelar fila, remover modelo, sobrescrever exportação |

## 3. Telas (fluxo)

### T1 — Biblioteca / Fila (home)
```
┌──────────────────────────────────────────────────────────────┐
│ ⬒ Melhorador de Textos                    [Config ⚙] [ + ]   │
├──────────────────────────────────────────────────────────────┤
│  ┌───────────── arraste PDFs ou uma pasta aqui ────────────┐ │
│  │                  ⭳  ou clique para escolher             │ │
│  └──────────────────────────────────────────────────────────┘ │
│  Destino: ~/Documentos/Livros Limpos            [Alterar…]    │
│  FILA (2 processando · 1 aguardando)          [▶ Iniciar tudo]│
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ 📕 As Primeiras Civilizações        OCR   [██████░░] 62% │ │
│  │    extração ✓ · limpeza ✓ · estrutura ▶ · revisão ·      │ │
│  │ 📗 Paideia                        nativo  aguardando  ⋯  │ │
│  └──────────────────────────────────────────────────────────┘ │
│  CONCLUÍDOS                                                   │
│  │ 📘 Schopenhauer I   ✓ 41 avisos   [Revisar] [Exportar]    │ │
└──────────────────────────────────────────────────────────────┘
```

### T2 — Detalhe do livro
Preview do texto (serif, 68ch) + coluna direita: metadados (autor/título/ISBN detectados + confiança), estatísticas do report, tabela de avisos clicável. CTA: **[Revisar sugestões]** / [Exportar].

### T3 — Revisão (diff)
Tela cheia do diff viewer (componente acima). Fonte das sugestões alternável em abas: `LanguageTool` · `IA local (Gemma 3 4B)` — cada aba com seu diff independente. Rodapé fixo: "Nada é aplicado sem sua aprovação · fonte intacta (sha256 …a1b2)".

### T4 — Exportação
Formato (.md / .txt / .docx) + destino + opções (incluir metadados de auditoria no rodapé; docx: mapa de estilos). Botão primário Exportar → toast com "Mostrar no Finder/Explorer".

### T5 — Configurações
Abas: **Geral** (idiomas OCR, faixa-amostra vs livro inteiro, tema; **sem pastas fixas** — origem/destino são perguntados no fluxo e o app apenas lembra as últimas escolhas; botão "Limpar dados temporários" do app-data) · **LanguageTool** (conta Premium: username+apiKey no chaveiro; servidor local: URL opcional; aviso claro de nuvem) · **Modelos de IA** (catálogo com cards, download/remover/ativar; aviso de RAM) · **Sobre**.

## 4. Estados e mensagens

- **Vazio:** ilustração leve de livro + "Arraste um PDF para começar".
- **Processando:** fila viva; janela pode ser fechada? Não no v1 — confirmar cancelamento.
- **Falha:** card fica `--err` com a causa em 1 linha + [Detalhes] (log) + [Tentar de novo]; fila **para** no livro que falhou (fail-fast, coerente com o CLI).
- **Idioma da UI:** pt-BR no v1 (strings centralizadas para i18n futura).
- Toda mensagem: resultado primeiro, sem jargão ("Não consegui ler as páginas 12–14" e não "PDFium error 0x03").

## 5. Acessibilidade e teclado

- Navegação completa por teclado; foco visível (anel accent 2px).
- Diff: `↑↓` navega, `⏎` aceita, `⌫` rejeita, `⌘Z` desfaz.
- `⌘O` abrir · `⌘E` exportar · `⌘,` configurações.
- Textos escaláveis (rem); modo escuro respeita o SO.

## 6. Implementação (E2)

- Tokens → `_APP/src/styles/tokens.css` (custom properties, tema por `data-theme`).
- shadcn/ui como base dos primitivos; componentes do DS em `_APP/src/components/ds/`.
- Storybook leve (ou página `/ds` interna) para revisão visual dos componentes.
- Este documento é a SSOT do DS: mudou aqui → muda no código, nunca o contrário.
