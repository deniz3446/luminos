#!/usr/bin/env bash
set -euo pipefail

PHOTOOS_DIR="${PHOTOOS_DIR:-$HOME/PhotoOS}"
STAMP="$(date +%Y%m%d-%H%M%S)"
BACKUP_DIR="$HOME/photoos-backups/storage-dashboard-$STAMP"

if [[ ! -d "$PHOTOOS_DIR/client" ]]; then
    echo "HATA: PhotoOS client dizini bulunamadı: $PHOTOOS_DIR/client"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

mkdir -p "$BACKUP_DIR/client/src/pages"
cp "$PHOTOOS_DIR/client/src/pages/StoragePage.tsx" "$BACKUP_DIR/client/src/pages/"
cp "$PHOTOOS_DIR/client/src/pages/StoragePage.css" "$BACKUP_DIR/client/src/pages/"

echo "Yedek oluşturuldu: $BACKUP_DIR"

cp "$SCRIPT_DIR/client/src/pages/StoragePage.tsx" "$PHOTOOS_DIR/client/src/pages/StoragePage.tsx"
cp "$SCRIPT_DIR/client/src/pages/StoragePage.css" "$PHOTOOS_DIR/client/src/pages/StoragePage.css"

echo "Frontend dosyaları güncellendi."
cd "$PHOTOOS_DIR/client"

npm run build

echo "Frontend derlemesi tamamlandı."

if systemctl list-unit-files | grep -q '^photoos-client.service'; then
    sudo systemctl restart photoos-client.service
    sleep 2
    sudo systemctl status photoos-client.service --no-pager -l
else
    echo "photoos-client.service bulunamadı; Vite geliştirme sunucusunu elle yeniden başlatın."
fi

echo
echo "API testleri:"
curl -fsS "http://127.0.0.1:8080/api/v1/storage/history?range=1h" >/dev/null && echo "✓ storage/history"
curl -fsS "http://127.0.0.1:8080/api/v1/storage/alerts" >/dev/null && echo "✓ storage/alerts"

echo
echo "Kurulum tamamlandı. Web paneli: http://192.168.1.51:5173"
