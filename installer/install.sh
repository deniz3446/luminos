#!/usr/bin/env bash
set -Eeuo pipefail

SELF_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$SELF_DIR/lib.sh"
# shellcheck disable=SC1091
source "$SELF_DIR/detect_system.sh"

VERSION="${PHOTOOS_VERSION:-1.0.0}"
PAYLOAD_DIR="${PHOTOOS_PAYLOAD_DIR:-$SELF_DIR/../payload}"

OPT_ROOT="/opt/photoos"
RELEASE_ROOT="$OPT_ROOT/releases"
RELEASE_DIR="$RELEASE_ROOT/$VERSION"
CURRENT_LINK="$OPT_ROOT/current"

ETC_ROOT="/etc/photoos"
VAR_ROOT="/var/lib/photoos"
RUNTIME_DIR="$VAR_ROOT/runtime"
UPDATE_ROOT="$VAR_ROOT/update"
PACKAGE_DIR="$UPDATE_ROOT/packages"
HISTORY_DIR="$UPDATE_ROOT/history"
VERIFICATION_DIR="$UPDATE_ROOT/verification"
STAGING_DIR="$UPDATE_ROOT/staging"
STORAGE_ROOT="/srv/luminos"
TOKEN_ENV_FILE="$ETC_ROOT/update-agent.env"

UPDATE_AGENT_DIR="$OPT_ROOT/update-agent"

install_dependencies() {
    log "Gerekli sistem paketleri kuruluyor..."
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt-get install -y --no-install-recommends \
        ca-certificates curl python3 sqlite3 tar util-linux
    ok "Bağımlılıklar hazır."
}

ensure_photoos_user() {
    if ! getent group photoos >/dev/null 2>&1; then
        groupadd --system photoos
    fi
    if ! id photoos >/dev/null 2>&1; then
        useradd --system --create-home --shell /bin/bash --gid photoos photoos
    fi
}

create_layout() {
    log "PhotoOS dizinleri oluşturuluyor..."
    install -d -m 0755 "$OPT_ROOT" "$RELEASE_ROOT" "$ETC_ROOT"
    install -d -m 0755 "$VAR_ROOT" "$RUNTIME_DIR" "$UPDATE_ROOT"
    install -d -o photoos -g photoos -m 0750 "$UPDATE_ROOT" "$PACKAGE_DIR" "$HISTORY_DIR" "$VERIFICATION_DIR" "$STAGING_DIR"
    install -d -m 0755 "$STORAGE_ROOT" "$UPDATE_AGENT_DIR"
    ok "Dizin yapısı hazır."
}

install_release() {
    [[ -d "$PAYLOAD_DIR/release" ]] || die "Release payload bulunamadı: $PAYLOAD_DIR/release"
    [[ -x "$PAYLOAD_DIR/release/server" ]] || die "Release içinde çalıştırılabilir server bulunamadı."

    log "PhotoOS $VERSION release kuruluyor..."

    local temp_release="${RELEASE_DIR}.installing"
    rm -rf "$temp_release"
    install -d -m 0755 "$temp_release"
    cp -a "$PAYLOAD_DIR/release/." "$temp_release/"

    printf '%s\n' "$VERSION" > "$temp_release/VERSION"
    chmod 0755 "$temp_release/server"

    rm -rf "$RELEASE_DIR"
    mv "$temp_release" "$RELEASE_DIR"
    atomic_symlink "$RELEASE_DIR" "$CURRENT_LINK"

    ok "Aktif release: $RELEASE_DIR"
}

install_update_agent() {
    for file in update_agent.py update_helper.py agent_core.py; do
        [[ -f "$PAYLOAD_DIR/update-agent/$file" ]] ||
            die "Update Agent payload eksik: $file"
    done

    log "Update Agent kuruluyor..."
    rm -rf "$UPDATE_AGENT_DIR"
    install -d -m 0755 "$UPDATE_AGENT_DIR"
    install -o root -g root -m 0755 "$PAYLOAD_DIR/update-agent/update_agent.py" "$UPDATE_AGENT_DIR/update_agent.py"
    install -o root -g root -m 0755 "$PAYLOAD_DIR/update-agent/update_helper.py" "$UPDATE_AGENT_DIR/update_helper.py"
    install -o root -g root -m 0644 "$PAYLOAD_DIR/update-agent/agent_core.py" "$UPDATE_AGENT_DIR/agent_core.py"
    ok "Update Agent hazır."
}

provision_update_token() {
    log "Update Agent anahtarı hazırlanıyor..."
    install -d -m 0755 "$ETC_ROOT"

    if [[ -f "$TOKEN_ENV_FILE" ]]; then
        grep -Eq '^PHOTOOS_UPDATE_TOKEN=[0-9a-f]{64}$' "$TOKEN_ENV_FILE" ||
            die "Mevcut Update Agent anahtarı geçersiz: $TOKEN_ENV_FILE"
    else
        local token
        token="$(python3 - <<'PYTOKEN'
import secrets
print(secrets.token_hex(32))
PYTOKEN
)"
        umask 0077
        printf 'PHOTOOS_UPDATE_TOKEN=%s\n' "$token" > "$TOKEN_ENV_FILE"
        unset token
    fi

    chmod 0640 "$TOKEN_ENV_FILE"
    chown root:photoos "$TOKEN_ENV_FILE"
    ok "Update Agent anahtarı hazır."
}


write_config() {
    log "Varsayılan yapılandırma yazılıyor..."

    if [[ ! -f "$ETC_ROOT/config.toml" ]]; then
        cat > "$ETC_ROOT/config.toml" <<EOF
[server]
host = "0.0.0.0"
port = 8080

[storage]
root = "$STORAGE_ROOT"

[database]
engine = "sqlite"
path = "$RUNTIME_DIR/luminos.db"

[logging]
level = "info"
EOF
    else
        warn "$ETC_ROOT/config.toml zaten var; üzerine yazılmadı."
    fi

    ok "Yapılandırma hazır."
}

write_services() {
    log "systemd servisleri kuruluyor..."

    cat > /etc/systemd/system/photoos.service <<'EOF'
[Unit]
Description=PhotoOS Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=root
Group=root
WorkingDirectory=/var/lib/photoos/runtime
Environment=PHOTOOS_CONFIG=/etc/photoos/config.toml
ExecStart=/opt/photoos/current/server
Restart=on-failure
RestartSec=3
NoNewPrivileges=true
PrivateTmp=true
ProtectHome=true
ReadWritePaths=/var/lib/photoos /srv/luminos /opt/photoos /etc/photoos

[Install]
WantedBy=multi-user.target
EOF

    for unit in photoos-update-helper.service photoos-update-agent.service; do
        [[ -f "$PAYLOAD_DIR/systemd/$unit" ]] ||
            die "Update Agent systemd payload eksik: $unit"
        install -o root -g root -m 0644 \
            "$PAYLOAD_DIR/systemd/$unit" \
            "/etc/systemd/system/$unit"
    done

    systemctl daemon-reload
    systemctl enable photoos.service photoos-update-helper.service photoos-update-agent.service
    ok "Servis dosyaları hazır."
}

start_and_verify() {
    log "PhotoOS servisleri başlatılıyor..."
    systemctl restart photoos.service
    systemctl restart photoos-update-helper.service
    systemctl restart photoos-update-agent.service

    local attempt
    for attempt in {1..30}; do
        if curl -fsS --max-time 2 \
            http://127.0.0.1:8080/api/v1/system/health >/dev/null 2>&1 &&
           curl -fsS --max-time 2 \
            http://127.0.0.1:8091/status >/dev/null 2>&1; then
            ok "PhotoOS ve Update Agent sağlık kontrolü başarılı."
            return 0
        fi
        sleep 1
    done

    systemctl status photoos.service --no-pager -l || true
    journalctl -u photoos.service -n 80 --no-pager || true
    die "PhotoOS sağlık kontrolü başarısız."
}

show_result() {
    local ip
    ip="$(hostname -I 2>/dev/null | awk '{print $1}')"
    echo
    echo "================================================"
    echo " PhotoOS kurulumu tamamlandı"
    echo "================================================"
    echo " Sürüm       : $VERSION"
    echo " Aktif release: $(readlink -f "$CURRENT_LINK")"
    echo " Web adresi  : http://${ip:-SUNUCU_IP}:8080"
    echo " Depolama    : $STORAGE_ROOT"
    echo " Yapılandırma: $ETC_ROOT/config.toml"
    echo
    echo " Servis kontrolü:"
    echo "   systemctl status photoos.service"
    echo "   systemctl status photoos-update-helper.service"
    echo "   systemctl status photoos-update-agent.service"
    echo
}

main() {
    require_root

    exec > >(tee -a /var/log/photoos-installer.log) 2>&1

    echo "================================================"
    echo " PhotoOS Installer $VERSION"
    echo "================================================"

    detect_system
    show_disks
    install_dependencies
    ensure_photoos_user
    create_layout
    install_release
    install_update_agent
    write_config
    provision_update_token
    write_services
    start_and_verify
    show_result
}

main "$@"
