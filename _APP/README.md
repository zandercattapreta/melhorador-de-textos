# _APP — App Desktop (Tauri 2)

Aplicativo multiplataforma (macOS/Linux/Windows): UI **TypeScript/React**, core **Rust** (100% compilado, sem dependências externas). Origem e destino dos arquivos são sempre escolhidos pelo usuário — o app não usa pastas fixas.

## Estrutura

```
src/          # UI React/TS (Vite)
src-tauri/    # casca Tauri (Rust)
core/         # crate melhorador-core (port Rust do pipeline — em andamento)
```

## Rodar em desenvolvimento

```bash
npm install
npm run tauri dev
```

## Referências

- Plano: [`../_docs/PLANO-APP-MELHORADOR.md`](../_docs/PLANO-APP-MELHORADOR.md)
- Design System (SSOT visual): [`../_docs/DESIGN-SYSTEM-APP.md`](../_docs/DESIGN-SYSTEM-APP.md)
- Backlog (épicos E0–E6): [`../_docs/BACKLOG-MELHORADOR.md`](../_docs/BACKLOG-MELHORADOR.md)

O CLI Python em [`../_CLI/`](../_CLI/) é a implementação de referência: o core Rust deve reproduzir suas saídas byte a byte (golden masters). Scaffold original do create-tauri-app: `README-tauri.md`.
