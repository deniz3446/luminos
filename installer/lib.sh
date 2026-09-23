#!/usr/bin/env bash
set -Eeuo pipefail

C_RESET='\033[0m'
C_RED='\033[31m'
C_GREEN='\033[32m'
C_YELLOW='\033[33m'
C_BLUE='\033[34m'

log()  { printf "${C_BLUE}[PhotoOS]${C_RESET} %s\n" "$*"; }
ok()   { printf "${C_GREEN}[  OK  ]${C_RESET} %s\n" "$*"; }
warn() { printf "${C_YELLOW}[ UYARI]${C_RESET} %s\n" "$*"; }
die()  { printf "${C_RED}[ HATA ]${C_RESET} %s\n" "$*" >&2; exit 1; }

require_root() {
    [[ ${EUID:-$(id -u)} -eq 0 ]] || die "Bu kurulum root yetkisi ister. sudo ile çalıştırın."
}

command_exists() {
    command -v "$1" >/dev/null 2>&1
}

atomic_symlink() {
    local target="$1"
    local link="$2"
    local tmp="${link}.new"
    ln -sfn "$target" "$tmp"
    mv -Tf "$tmp" "$link"
}

cleanup_on_error() {
    local exit_code=$?
    if [[ $exit_code -ne 0 ]]; then
        warn "Kurulum hata ile durdu. Ayrıntılar: /var/log/photoos-installer.log"
    fi
}
trap cleanup_on_error EXIT
