#!/usr/bin/env bash
# ==============================================================================
# SCRIPT: bundle-tessdata.sh
# DESCRIÇÃO: Copia tessdata (por/eng) para resources do app Tauri (A12)
# CHAMADO POR: build manual antes de `npm run tauri build`
# CONTRATO (RESPOSTA ESPERADA): _APP/src-tauri/tessdata/*.traineddata
# ==============================================================================
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DEST="$ROOT/_APP/src-tauri/tessdata"
mkdir -p "$DEST"
SRC="${TESSDATA_PREFIX:-/opt/homebrew/share/tessdata}"
if [[ ! -d "$SRC" ]]; then
  SRC="/usr/local/share/tessdata"
fi
for lang in por eng; do
  f="$SRC/${lang}.traineddata"
  if [[ -f "$f" ]]; then
    cp -f "$f" "$DEST/"
    echo "[ok] $lang -> $DEST"
  else
    echo "[warn] ausente: $f" >&2
  fi
done
echo "Adicione tessdata/ em tauri.conf.json bundle.resources se ainda não estiver."
