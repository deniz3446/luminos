#!/usr/bin/env bash
set -euo pipefail

PHOTOOS_DIR="${PHOTOOS_DIR:-$HOME/PhotoOS}"
PACKAGE_DIR="$(cd "$(dirname "$0")" && pwd)"
BACKUP_DIR="$HOME/photoos-backups/storage-history-$(date +%Y%m%d-%H%M%S)"

cd "$PHOTOOS_DIR"
mkdir -p "$BACKUP_DIR/server/src/handlers" "$BACKUP_DIR/server/src/routes"
cp server/src/main.rs "$BACKUP_DIR/server/src/main.rs"
cp server/src/handlers/mod.rs "$BACKUP_DIR/server/src/handlers/mod.rs"
cp server/src/routes/storage.rs "$BACKUP_DIR/server/src/routes/storage.rs"
[ -f server/src/handlers/storage_history.rs ] && cp server/src/handlers/storage_history.rs "$BACKUP_DIR/server/src/handlers/storage_history.rs" || true

cp "$PACKAGE_DIR/server/src/main.rs" server/src/main.rs
cp "$PACKAGE_DIR/server/src/handlers/mod.rs" server/src/handlers/mod.rs
cp "$PACKAGE_DIR/server/src/handlers/storage_history.rs" server/src/handlers/storage_history.rs
cp "$PACKAGE_DIR/server/src/routes/storage.rs" server/src/routes/storage.rs

echo "Yedek: $BACKUP_DIR"
echo "Kaynak dosyalar kuruldu. Şimdi derleniyor..."
cd server
cargo build --release

echo "Derleme tamamlandı. Binary yayına alınıyor..."
sudo systemctl stop photoos.service
sudo cp target/release/server /opt/photoos/current/server
sudo chmod +x /opt/photoos/current/server
sudo systemctl start photoos.service
sudo systemctl status photoos.service --no-pager

echo
echo "API testleri:"
sleep 2
curl -fsS 'http://127.0.0.1:8080/api/v1/storage/history?range=1h' | python3 -m json.tool
curl -fsS 'http://127.0.0.1:8080/api/v1/storage/alerts' | python3 -m json.tool
