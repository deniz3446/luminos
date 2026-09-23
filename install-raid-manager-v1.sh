#!/usr/bin/env bash
set -euo pipefail
PROJECT="$HOME/PhotoOS"
PACKAGE_DIR="$(cd "$(dirname "$0")" && pwd)"
STAMP="$(date +%Y%m%d-%H%M%S)"
BACKUP="$HOME/photoos-backups/raid-manager-v1-$STAMP"
mkdir -p "$BACKUP/server/src/handlers" "$BACKUP/server/src/routes" "$BACKUP/client/src/pages" "$BACKUP/client/src/layouts"
for f in server/src/app.rs server/src/handlers/mod.rs server/src/routes/mod.rs client/src/App.tsx client/src/layouts/DashboardLayout.tsx; do mkdir -p "$BACKUP/$(dirname "$f")"; cp "$PROJECT/$f" "$BACKUP/$f"; done
[ -f "$PROJECT/server/src/handlers/raid.rs" ] && cp "$PROJECT/server/src/handlers/raid.rs" "$BACKUP/server/src/handlers/" || true
[ -f "$PROJECT/server/src/routes/raid.rs" ] && cp "$PROJECT/server/src/routes/raid.rs" "$BACKUP/server/src/routes/" || true
[ -f "$PROJECT/client/src/pages/RaidManagerPage.tsx" ] && cp "$PROJECT/client/src/pages/RaidManagerPage.tsx" "$BACKUP/client/src/pages/" || true
[ -f "$PROJECT/client/src/pages/RaidManagerPage.css" ] && cp "$PROJECT/client/src/pages/RaidManagerPage.css" "$BACKUP/client/src/pages/" || true

echo "Yedek: $BACKUP"
command -v mdadm >/dev/null || { echo "mdadm kuruluyor..."; sudo apt-get update; sudo DEBIAN_FRONTEND=noninteractive apt-get install -y mdadm; }
cp "$PACKAGE_DIR/server/src/handlers/raid.rs" "$PROJECT/server/src/handlers/raid.rs"
cp "$PACKAGE_DIR/server/src/routes/raid.rs" "$PROJECT/server/src/routes/raid.rs"
cp "$PACKAGE_DIR/client/src/pages/RaidManagerPage.tsx" "$PROJECT/client/src/pages/RaidManagerPage.tsx"
cp "$PACKAGE_DIR/client/src/pages/RaidManagerPage.css" "$PROJECT/client/src/pages/RaidManagerPage.css"
python3 - <<'PY'
from pathlib import Path
p=Path.home()/"PhotoOS"
def edit(rel,fn):
 f=p/rel;t=f.read_text();f.write_text(fn(t))
edit("server/src/handlers/mod.rs",lambda t:t if "pub mod raid;" in t else t.rstrip()+"\npub mod raid;\n")
edit("server/src/routes/mod.rs",lambda t:t if "pub mod raid;" in t else t.rstrip()+"\npub mod raid;\n")
def app(t):
 if ".merge(crate::routes::raid::router())" not in t:t=t.replace(".merge(storage::router())", ".merge(storage::router())\n        .merge(crate::routes::raid::router())")
 return t
edit("server/src/app.rs",app)
def ap(t):
 if 'RaidManagerPage' not in t:t=t.replace('import StoragePage from "./pages/StoragePage";', 'import StoragePage from "./pages/StoragePage";\nimport RaidManagerPage from "./pages/RaidManagerPage";')
 if 'path="/raid"' not in t:t=t.replace('<Route path="/storage" element={<StoragePage />} />','<Route path="/storage" element={<StoragePage />} />\n                    <Route path="/raid" element={<RaidManagerPage />} />')
 return t
edit("client/src/App.tsx",ap)
def layout(t):
 if 'to="/raid"' not in t:t=t.replace('<NavLink to="/storage">💾 Depolama</NavLink>','<NavLink to="/storage">💾 Depolama</NavLink>\n                            <NavLink to="/raid">🛡️ RAID Manager</NavLink>')
 return t
edit("client/src/layouts/DashboardLayout.tsx",layout)
PY
cd "$PROJECT"
echo "Backend derleniyor..."
cargo build --release
echo "Frontend derleniyor..."
cd "$PROJECT/client" && npm run build
echo "Binary yayına alınıyor..."
sudo systemctl stop photoos.service
sudo cp "$PROJECT/target/release/server" /opt/photoos/current/server
sudo chmod +x /opt/photoos/current/server
sudo systemctl start photoos.service
sleep 3
curl -fsS http://127.0.0.1:8080/api/v1/raid/status >/dev/null
curl -fsS http://127.0.0.1:8080/api/v1/raid/candidates >/dev/null
echo "RAID Manager V1 kuruldu: http://192.168.1.51:5173/raid"
echo "Bu sürüm güvenli planlama ve izleme modundadır; disk silen işlem çalıştırmaz."
