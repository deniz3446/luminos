#!/usr/bin/env bash
set -Eeuo pipefail

PROJECT_ROOT="${1:-$HOME/PhotoOS}"
VERSION="${2:-1.0.0}"

SOURCE_RELEASE="${PHOTOOS_SOURCE_RELEASE:-/opt/photoos/releases/$VERSION}"
SOURCE_AGENT="$PROJECT_ROOT/update-agent"
SOURCE_SYSTEMD="$PROJECT_ROOT/systemd"

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
WORK_ROOT="$PROJECT_ROOT/dist/installer-work-$VERSION"
PAYLOAD="$WORK_ROOT/payload"
OUTPUT="$PROJECT_ROOT/dist/PhotoOS-Installer-$VERSION.run"

[[ -d "$SOURCE_RELEASE" ]] || {
    echo "Release bulunamadı: $SOURCE_RELEASE" >&2
    exit 1
}
[[ -x "$SOURCE_RELEASE/server" ]] || {
    echo "Server binary bulunamadı veya çalıştırılabilir değil." >&2
    exit 1
}

for file in update_agent.py update_helper.py agent_core.py; do
    [[ -f "$SOURCE_AGENT/$file" ]] || {
        echo "Update Agent canonical kaynak eksik: $SOURCE_AGENT/$file" >&2
        exit 1
    }
done

for unit in photoos-update-agent.service photoos-update-helper.service; do
    [[ -f "$SOURCE_SYSTEMD/$unit" ]] || {
        echo "Update Agent systemd unit eksik: $SOURCE_SYSTEMD/$unit" >&2
        exit 1
    }
done

rm -rf "$WORK_ROOT"
mkdir -p "$PAYLOAD/release" "$PAYLOAD/update-agent" "$PAYLOAD/systemd" "$PROJECT_ROOT/dist"

cp -a "$SOURCE_RELEASE/." "$PAYLOAD/release/"
for file in update_agent.py update_helper.py agent_core.py; do
    cp -a "$SOURCE_AGENT/$file" "$PAYLOAD/update-agent/$file"
done
for unit in photoos-update-agent.service photoos-update-helper.service; do
    cp -a "$SOURCE_SYSTEMD/$unit" "$PAYLOAD/systemd/$unit"
done
cp -a "$SCRIPT_DIR/." "$WORK_ROOT/installer/"

# Geçici veya istenmeyen dosyaları paket dışı bırak.
find "$PAYLOAD" -type f \
    \( -name '*.bak' -o -name '*.tmp' -o -name '*.pyc' \) -delete
find "$PAYLOAD" -type d -name '__pycache__' -prune -exec rm -rf {} +

printf '%s\n' "$VERSION" > "$PAYLOAD/release/VERSION"

ARCHIVE="$WORK_ROOT/payload.tar.gz"
tar -C "$WORK_ROOT" -czf "$ARCHIVE" installer payload

cat > "$OUTPUT" <<'STUB'
#!/usr/bin/env bash
set -Eeuo pipefail

MARKER="__PHOTOOS_ARCHIVE_BELOW__"
SELF="$0"
TMPDIR="$(mktemp -d /tmp/photoos-installer.XXXXXX)"

cleanup() {
    rm -rf "$TMPDIR"
}
trap cleanup EXIT

LINE="$(awk "/^${MARKER}$/ {print NR + 1; exit}" "$SELF")"
[[ -n "$LINE" ]] || {
    echo "Installer arşiv işaretçisi bulunamadı." >&2
    exit 1
}

tail -n +"$LINE" "$SELF" | tar -xzf - -C "$TMPDIR"
export PHOTOOS_PAYLOAD_DIR="$TMPDIR/payload"
export PHOTOOS_VERSION="__VERSION__"
exec "$TMPDIR/installer/install.sh" "$@"
__PHOTOOS_ARCHIVE_BELOW__
STUB

sed -i "s/__VERSION__/$VERSION/g" "$OUTPUT"
cat "$ARCHIVE" >> "$OUTPUT"
chmod 0755 "$OUTPUT"

echo
echo "Installer oluşturuldu:"
echo "$OUTPUT"
echo
sha256sum "$OUTPUT"
