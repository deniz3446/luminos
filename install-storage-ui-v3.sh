#!/usr/bin/env bash
set -euo pipefail
PROJECT="${PHOTOOS_DIR:-$HOME/PhotoOS}"
SRC="$(cd "$(dirname "$0")" && pwd)"
STAMP="$(date +%Y%m%d-%H%M%S)"
BACKUP="$HOME/photoos-backups/storage-ui-v3-$STAMP"
mkdir -p "$BACKUP"
cp "$PROJECT/client/src/pages/StoragePage.tsx" "$BACKUP/"
cp "$PROJECT/client/src/pages/StoragePage.css" "$BACKUP/"
cp "$SRC/client/src/pages/StoragePage.tsx" "$PROJECT/client/src/pages/StoragePage.tsx"
cp "$SRC/client/src/pages/StoragePage.css" "$PROJECT/client/src/pages/StoragePage.css"
echo "Yedek: $BACKUP"
cd "$PROJECT/client"
npm run build
echo
echo "Storage UI V3 kuruldu. Vite açıksa Ctrl+F5 yapın."
