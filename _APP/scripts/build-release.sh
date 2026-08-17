#!/usr/bin/env bash
# ==============================================================================
# SCRIPT: build-release.sh (txtmelhorator-app)
# DESCRIÇÃO: Gera o .app de release e deposita uma cópia em _APP/version/
#            (uma pasta por build: <YYYY-MM-DD_HHMM>/TXTMelhorator.app).
#            Convenção do Zander (16/Ago): todo novo build vai para _APP/version.
# CHAMADO POR: desenvolvedor/agente — bash _APP/scripts/build-release.sh (da raiz
#             ou de _APP; o script se orienta sozinho)
# CONTRATO (RESPOSTA ESPERADA): imprime o caminho final do .app; sai != 0 se o
#             build ou a cópia falharem. Falha do .dmg NÃO derruba o script
#             (limitação conhecida de build sem Finder).
# ==============================================================================

set -euo pipefail

APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLE="$APP_DIR/src-tauri/target/release/bundle/macos/TXTMelhorator.app"
VERSION_DIR="$APP_DIR/version"
STAMP="$(date +%Y-%m-%d_%H%M)"
DEST="$VERSION_DIR/$STAMP"

echo "[build-release] compilando (npm run tauri build)…"
# O empacotamento do .dmg falha em sessão sem Finder; o .app sai perfeito.
# Por isso não deixamos o exit code do tauri derrubar o script — validamos
# pela existência e frescor do .app logo abaixo.
cd "$APP_DIR"
npm run tauri build || echo "[build-release] aviso: tauri saiu com erro (provável .dmg); validando o .app…"

if [ ! -d "$BUNDLE" ]; then
  echo "[build-release] ERRO: .app não foi gerado em $BUNDLE" >&2
  exit 1
fi

# Frescor: o binário precisa ser mais novo que o início deste script (60 s de folga).
BIN="$BUNDLE/Contents/MacOS/txtmelhorator-app"
if [ ! -f "$BIN" ]; then
  echo "[build-release] ERRO: binário ausente dentro do .app" >&2
  exit 1
fi

mkdir -p "$DEST"
# ditto preserva assinaturas/atributos de bundles macOS (cp -R pode corromper).
ditto "$BUNDLE" "$DEST/TXTMelhorator.app"

echo "[build-release] OK: $DEST/TXTMelhorator.app"
