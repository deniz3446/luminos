#!/usr/bin/env bash
set -euo pipefail
PROJECT="$HOME/PhotoOS"
PACKAGE_DIR="$(cd "$(dirname "$0")" && pwd)"
STAMP="$(date +%Y%m%d-%H%M%S)"
BACKUP="$HOME/photoos-backups/backup-manager-$STAMP"
mkdir -p "$BACKUP/server/src/handlers" "$BACKUP/server/src/routes" "$BACKUP/client/src/pages"

for f in server/src/main.rs server/src/app.rs server/src/handlers/mod.rs server/src/routes/mod.rs client/src/App.tsx client/src/layouts/DashboardLayout.tsx; do
  mkdir -p "$BACKUP/$(dirname "$f")"; cp "$PROJECT/$f" "$BACKUP/$f"
done
[ -f "$PROJECT/client/src/pages/BackupManagerPage.tsx" ] && cp "$PROJECT/client/src/pages/BackupManagerPage.tsx" "$BACKUP/client/src/pages/" || true
[ -f "$PROJECT/client/src/pages/BackupManagerPage.css" ] && cp "$PROJECT/client/src/pages/BackupManagerPage.css" "$BACKUP/client/src/pages/" || true

echo "Yedek oluşturuldu: $BACKUP"
command -v rsync >/dev/null || { echo "rsync kuruluyor..."; sudo apt-get update; sudo apt-get install -y rsync; }
cp "$PACKAGE_DIR/server/src/handlers/backups.rs" "$PROJECT/server/src/handlers/backups.rs"
cp "$PACKAGE_DIR/server/src/routes/backups.rs" "$PROJECT/server/src/routes/backups.rs"
cp "$PACKAGE_DIR/client/src/pages/BackupManagerPage.tsx" "$PROJECT/client/src/pages/BackupManagerPage.tsx"
cp "$PACKAGE_DIR/client/src/pages/BackupManagerPage.css" "$PROJECT/client/src/pages/BackupManagerPage.css"

python3 - <<'PY'
from pathlib import Path
p=Path.home()/"PhotoOS"

def edit(rel, fn):
    f=p/rel; t=f.read_text(); n=fn(t); f.write_text(n)

edit("server/src/handlers/mod.rs", lambda t: t if "pub mod backups;" in t else t.rstrip()+"\npub mod backups;\n")
edit("server/src/routes/mod.rs", lambda t: t if "pub mod backups;" in t else t.rstrip()+"\npub mod backups;\n")

def app(t):
    t=t.replace("use crate::routes::{devices, media, photos, storage, system, user};","use crate::routes::{backups, devices, media, photos, storage, system, user};")
    if ".merge(backups::router())" not in t: t=t.replace(".merge(storage::router())", ".merge(storage::router())\n        .merge(backups::router())")
    return t
edit("server/src/app.rs",app)

def main(t):
    marker="    handlers::storage_history::spawn_storage_history_collector(state.db.clone());"
    insertion='''    handlers::backups::initialize_backup_manager(&state.db)\n        .await\n        .expect("Backup Manager tablosu oluşturulamadı");\n'''
    if "initialize_backup_manager" not in t: t=t.replace(marker, marker+"\n"+insertion)
    return t
edit("server/src/main.rs",main)

def apptsx(t):
    if 'BackupManagerPage' not in t: t=t.replace('import StoragePage from "./pages/StoragePage";', 'import StoragePage from "./pages/StoragePage";\nimport BackupManagerPage from "./pages/BackupManagerPage";')
    if 'path="/backups"' not in t: t=t.replace('<Route path="/storage" element={<StoragePage />} />', '<Route path="/storage" element={<StoragePage />} />\n                    <Route path="/backups" element={<BackupManagerPage />} />')
    return t
edit("client/src/App.tsx", apptsx)

def layout(t):
    if 'to="/backups"' not in t: t=t.replace('<NavLink to="/storage">💾 Depolama</NavLink>', '<NavLink to="/storage">💾 Depolama</NavLink>\n                            <NavLink to="/backups">🛡️ Yedekleme</NavLink>')
    return t
edit("client/src/layouts/DashboardLayout.tsx", layout)
PY

echo "Backend derleniyor..."
cd "$PROJECT"
cargo build --release

echo "Frontend derleniyor..."
cd "$PROJECT/client"
npm run build

echo "Binary yayına alınıyor..."
sudo systemctl stop photoos.service
sudo cp "$PROJECT/target/release/server" /opt/photoos/current/server
sudo chmod +x /opt/photoos/current/server
sudo systemctl start photoos.service
sleep 3

curl -fsS http://127.0.0.1:8080/api/v1/backups/status >/dev/null
curl -fsS http://127.0.0.1:8080/api/v1/backups/targets >/dev/null

echo "Backup Manager kuruldu."
echo "Web: http://192.168.1.51:5173/backups"
echo "Vite açıksa Ctrl+F5 yapın; kapalıysa istemciyi yeniden başlatın."
