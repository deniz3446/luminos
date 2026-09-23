#!/usr/bin/env bash
set -Eeuo pipefail

detect_system() {
    log "İşletim sistemi ve donanım kontrol ediliyor..."

    [[ -r /etc/os-release ]] || die "/etc/os-release bulunamadı."
    # shellcheck disable=SC1091
    source /etc/os-release

    case "${ID:-}" in
        debian)
            ;;
        ubuntu)
            warn "Ubuntu algılandı. İlk test hedefimiz Debian 13; kurulum devam edecek."
            ;;
        *)
            die "Desteklenmeyen dağıtım: ${PRETTY_NAME:-bilinmiyor}"
            ;;
    esac

    local arch
    arch="$(dpkg --print-architecture 2>/dev/null || uname -m)"
    case "$arch" in
        amd64|x86_64) ;;
        *) die "Bu paket yalnızca amd64/x86_64 için hazırlandı. Algılanan: $arch" ;;
    esac

    local ram_kb
    ram_kb="$(awk '/MemTotal:/ {print $2}' /proc/meminfo)"
    if (( ram_kb < 1900000 )); then
        warn "2 GB altında RAM algılandı. PhotoOS yavaş çalışabilir."
    fi

    local free_kb
    free_kb="$(df -Pk /opt 2>/dev/null | awk 'NR==2 {print $4}')"
    if [[ -n "$free_kb" ]] && (( free_kb < 1048576 )); then
        die "/opt altında en az 1 GB boş alan gerekli."
    fi

    ok "Sistem: ${PRETTY_NAME:-bilinmiyor}"
    ok "Mimari: $arch"
}

show_disks() {
    log "Takılı diskler algılanıyor (salt okunur kontrol)..."
    echo
    lsblk -e7 -o NAME,PATH,TYPE,SIZE,MODEL,SERIAL,FSTYPE,MOUNTPOINTS
    echo
    warn "Bu installer hiçbir diski biçimlendirmez veya RAID oluşturmaz."
}
