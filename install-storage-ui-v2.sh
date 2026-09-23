#!/usr/bin/env bash
set -euo pipefail

PROJECT="$HOME/PhotoOS"
SOURCE_DIR="$(cd "$(dirname "$0")" && pwd)"
BACKUP_DIR="$HOME/photoos-backups/storage-ui-v2-$(date +%Y%m%d-%H%M%S)"

if [ ! -d "$PROJECT/client/src/pages" ]; then
  echo "HATA: $PROJECT/client/src/pages bulunamadı."
  exit 1
fi

mkdir -p "$BACKUP_DIR"
cp "$PROJECT/client/src/pages/StoragePage.css" "$BACKUP_DIR/StoragePage.css"
cp "$SOURCE_DIR/client/src/pages/StoragePage.css" "$PROJECT/client/src/pages/StoragePage.css"

echo "Yedek: $BACKUP_DIR"
echo "Yeni Storage UI dosyası kuruldu."

cd "$PROJECT/client"
npm run build

echo
echo "Kurulum tamamlandı."
echo "Vite açıksa değişiklik otomatik uygulanır. Tarayıcıda Ctrl+F5 yapın."
