#!/usr/bin/env bash
# ==============================================================================
# SCRIPT: notarize-macos.sh
# DESCRIÇÃO: Assina + notariza o .app/.dmg (requer Apple Developer)
# CHAMADO POR: release manual
# CONTRATO (RESPOSTA ESPERADA): exit 0 após stapling; falha se env ausente
# ==============================================================================
set -euo pipefail
: "${APPLE_ID:?defina APPLE_ID}"
: "${APPLE_TEAM_ID:?defina APPLE_TEAM_ID}"
: "${APPLE_APP_PASSWORD:?defina APPLE_APP_PASSWORD (app-specific)}"
APP_PATH="${1:?uso: $0 caminho/do.app}"

codesign --force --deep --options runtime \
  --sign "Developer ID Application: ${APPLE_TEAM_ID}" \
  "$APP_PATH"

ZIP="${APP_PATH}.zip"
ditto -c -k --keepParent "$APP_PATH" "$ZIP"
xcrun notarytool submit "$ZIP" \
  --apple-id "$APPLE_ID" \
  --team-id "$APPLE_TEAM_ID" \
  --password "$APPLE_APP_PASSWORD" \
  --wait
xcrun stapler staple "$APP_PATH"
echo "[ok] notarizado: $APP_PATH"
