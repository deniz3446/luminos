#!/usr/bin/env bash
set -Eeuo pipefail

export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

TARGET="${PHOTOOS_TARGET_DISK:-}"
MNT="/mnt/photoos-target"

PHOTOOS_INSTALLER3="${PHOTOOS_INSTALLER3:-0}"
PHOTOOS_HOSTNAME="${PHOTOOS_HOSTNAME:-photoos}"
PHOTOOS_TIMEZONE="${PHOTOOS_TIMEZONE:-Europe/Istanbul}"
PHOTOOS_SERVER_NAME="${PHOTOOS_SERVER_NAME:-PhotoOS}"
PHOTOOS_SSH_PASSWORD_FILE="${PHOTOOS_SSH_PASSWORD_FILE:-}"
PHOTOOS_MIN_DISK_BYTES="${PHOTOOS_MIN_DISK_BYTES:-34359738368}"

fail() {
    echo
    echo "❌ HATA: $1"
    exit 1
}

cleanup() {
    umount "$MNT/boot/efi" 2>/dev/null || true
    umount "$MNT/dev/pts" 2>/dev/null || true
    umount "$MNT/dev" 2>/dev/null || true
    umount "$MNT/proc" 2>/dev/null || true
    umount "$MNT/sys" 2>/dev/null || true
    umount "$MNT/run" 2>/dev/null || true
    umount "$MNT" 2>/dev/null || true
}
trap cleanup EXIT

[[ "$(id -u)" -eq 0 ]] ||
    fail "Installer root olarak çalıştırılmalı."

echo "=================================================="
echo " PHOTOOS INSTALLER 2.0"
echo " GERÇEK DİSK KURULUMU"
echo "=================================================="

echo
echo "===== BOOT ORTAMI ====="

LIVE_MEDIUM="$(
    findmnt -n -o SOURCE /run/live/medium 2>/dev/null || true
)"

echo "Live medium: ${LIVE_MEDIUM:-bilinmiyor}"

LIVE_DISK=""

if [[ "$LIVE_MEDIUM" =~ ^/dev/ ]]; then
    LIVE_DISK="/dev/$(lsblk -no PKNAME "$LIVE_MEDIUM" 2>/dev/null || true)"
fi

echo "Live disk  : ${LIVE_DISK:-bilinmiyor}"

# Destructive installer live kaynagi kesin olarak tanimadan devam etmez.
[[ -n "$LIVE_DISK" && -b "$LIVE_DISK" ]] ||
    fail "Live kurulum diski guvenli sekilde algilanamadi. Disk silme islemi baslatilmadi."

echo
echo "===== UYGUN HEDEF DİSKLER ====="

mapfile -t DISKS < <(
    lsblk \
        -dpno NAME,TYPE,SIZE,MODEL,TRAN \
        | awk '$2=="disk" {print}'
)

VALID=()
INDEX=1

for LINE in "${DISKS[@]}"; do
    DEV="$(awk '{print $1}' <<< "$LINE")"

    if [[ -n "$LIVE_DISK" && "$DEV" == "$LIVE_DISK" ]]; then
        echo "ATLANDI (USB/Live): $LINE"
        continue
    fi

    VALID+=("$DEV")
    echo "[$INDEX] $LINE"
    ((INDEX++))
done

[[ "${#VALID[@]}" -gt 0 ]] ||
    fail "Kurulabilecek dahili disk bulunamadı."

echo

if [[ -n "$TARGET" ]]; then
    echo "Installer 3.0 hedef diski: $TARGET"

    [[ -b "$TARGET" ]] ||
        fail "Installer 3.0 hedefi block device değil: $TARGET"

    TARGET_TYPE="$(
        lsblk -ndo TYPE "$TARGET" 2>/dev/null |
        head -1
    )"

    [[ "$TARGET_TYPE" == "disk" ]] ||
        fail "Installer 3.0 hedefi fiziksel disk değil."

    if [[ -n "$LIVE_DISK" && "$TARGET" == "$LIVE_DISK" ]]; then
        fail "Live USB hedef disk olarak kullanılamaz."
    fi

    FOUND=0

    for DEV in "${VALID[@]}"; do
        if [[ "$DEV" == "$TARGET" ]]; then
            FOUND=1
            break
        fi
    done

    [[ "$FOUND" -eq 1 ]] ||
        fail "Seçilen hedef güvenli disk listesinde değil: $TARGET"

else
    echo "⚠️ Seçilecek diskin TÜM İÇERİĞİ SİLİNECEKTİR."
    echo

    read -rp "Disk numarası: " CHOICE

    [[ "$CHOICE" =~ ^[0-9]+$ ]] ||
        fail "Geçersiz seçim."

    (( CHOICE >= 1 && CHOICE <= ${#VALID[@]} )) ||
        fail "Geçersiz disk numarası."

    TARGET="${VALID[$((CHOICE-1))]}"
fi

echo
echo "Seçilen disk:"
lsblk -d -o NAME,SIZE,MODEL,SERIAL,TRAN "$TARGET"

TARGET_SIZE_BYTES="$(blockdev --getsize64 "$TARGET")"
[[ "$TARGET_SIZE_BYTES" =~ ^[0-9]+$ ]] ||
    fail "Hedef disk boyutu okunamadi."

(( TARGET_SIZE_BYTES >= PHOTOOS_MIN_DISK_BYTES )) ||
    fail "Hedef disk cok kucuk. Minimum 32 GiB gerekli."

echo "Disk boyutu guvenlik kontrolu: OK"

echo

if [[ "$PHOTOOS_INSTALLER3" == "1" ]]; then

    EXPECTED_CONFIRM="$TARGET"

    [[ "${PHOTOOS_ERASE_CONFIRM:-}" == "$EXPECTED_CONFIRM" ]] ||
        fail "Installer 3.0 disk silme güvenlik doğrulaması başarısız."

    echo "✅ Installer 3.0 disk silme onayı doğrulandı."

else

    read -rp "Devam etmek için TAM OLARAK ERASE yazın: " CONFIRM

    [[ "$CONFIRM" == "ERASE" ]] ||
        fail "Kurulum kullanıcı tarafından iptal edildi."
fi

echo
echo "===== DİSK TEMİZLENİYOR ====="

swapoff -a 2>/dev/null || true

umount "${TARGET}"?* 2>/dev/null || true
umount "${TARGET}"p?* 2>/dev/null || true

wipefs -a "$TARGET"
sgdisk --zap-all "$TARGET"

echo
echo "===== GPT BÖLÜMLENDİRME ====="

parted -s "$TARGET" \
    mklabel gpt \
    mkpart BIOSBOOT 1MiB 3MiB \
    set 1 bios_grub on \
    mkpart ESP fat32 3MiB 515MiB \
    set 2 esp on \
    mkpart PHOTOOS ext4 515MiB 100%

partprobe "$TARGET"
udevadm settle

if [[ "$TARGET" == *nvme* || "$TARGET" == *mmcblk* ]]; then
    BIOS="${TARGET}p1"
    EFI="${TARGET}p2"
    ROOT="${TARGET}p3"
else
    BIOS="${TARGET}1"
    EFI="${TARGET}2"
    ROOT="${TARGET}3"
fi

[[ -b "$EFI" ]] || fail "EFI partition oluşmadı."
[[ -b "$ROOT" ]] || fail "Root partition oluşmadı."

echo
echo "EFI : $EFI"
echo "ROOT: $ROOT"

echo
echo "===== DOSYA SİSTEMLERİ ====="

mkfs.vfat -F32 -n PHOTOOS_EFI "$EFI"
mkfs.ext4 -F -L PHOTOOS_ROOT "$ROOT"

mkdir -p "$MNT"
mount "$ROOT" "$MNT"

mkdir -p "$MNT/boot/efi"
mount "$EFI" "$MNT/boot/efi"

echo
echo "===== LIVE SİSTEM HEDEFE KOPYALANIYOR ====="

rsync -aHAXx --numeric-ids \
    --exclude=/dev/* \
    --exclude=/proc/* \
    --exclude=/sys/* \
    --exclude=/run/* \
    --exclude=/tmp/* \
    --exclude=/mnt/* \
    --exclude=/media/* \
   --exclude=/usr/lib/live/mount/* \
    --exclude=/lost+found \
    / "$MNT/"

mkdir -p \
    "$MNT/dev" \
    "$MNT/proc" \
    "$MNT/sys" \
    "$MNT/run" \
    "$MNT/tmp" \
    "$MNT/var/lib/photoos/runtime" \
    "$MNT/srv/photoos/disks" \
    "$MNT/srv/luminos"

chmod 1777 "$MNT/tmp"

echo
echo "===== FRESH INSTALL SANITIZATION ====="

# Her kurulum benzersiz sistem kimligi, SSH anahtarlari ve runtime state ile baslar.
rm -f "$MNT/etc/machine-id"
: > "$MNT/etc/machine-id"
rm -f "$MNT/var/lib/dbus/machine-id" 2>/dev/null || true

rm -f "$MNT/etc/ssh/ssh_host_"* 2>/dev/null || true

rm -rf     "$MNT/var/lib/photoos/runtime"     "$MNT/var/lib/photoos/storage"     "$MNT/var/lib/photoos/installer"     "$MNT/var/lib/photoos/installer-state"     "$MNT/var/log/photoos"

mkdir -p     "$MNT/var/lib/photoos/runtime"     "$MNT/var/lib/photoos/storage"     "$MNT/var/lib/photoos/installer"     "$MNT/var/lib/photoos/installer-state"     "$MNT/var/log/photoos"

echo "Fresh install runtime/state temizligi: OK"

echo
echo "===== FSTAB ====="

ROOT_UUID="$(blkid -s UUID -o value "$ROOT")"
EFI_UUID="$(blkid -s UUID -o value "$EFI")"

cat > "$MNT/etc/fstab" <<FSTAB
UUID=$ROOT_UUID / ext4 defaults,noatime 0 1
UUID=$EFI_UUID /boot/efi vfat umask=0077 0 1
FSTAB

cat "$MNT/etc/fstab"

echo
echo "===== LIVE INSTALLER TEMİZLİĞİ ====="

# Kurulu sistem Installer 3.x arayüzünü tekrar açmamalıdır.
systemctl --root="$MNT" disable photoos-installer3.service     >/dev/null 2>&1 || true

rm -f     "$MNT/etc/systemd/system/multi-user.target.wants/photoos-installer3.service"

echo "✅ Live Installer hedef sistemde devre dışı."

echo
echo "===== INSTALLED SYSTEM MARKER ====="

mkdir -p "$MNT/etc/photoos"

cat > "$MNT/etc/photoos/installed-system" <<EOF
product=PhotoOS
version=1.2.1-rc2
installed_at=$(date -Iseconds)
target_disk=$TARGET
installer=3.1
EOF

chmod 0644 "$MNT/etc/photoos/installed-system"

echo "✅ installed-system marker oluşturuldu."

echo
echo "===== FIRSTBOOT ENABLE ====="

if [[ -f "$MNT/etc/systemd/system/photoos-firstboot-installer.service" ]]; then
    systemctl --root="$MNT" enable         photoos-firstboot-installer.service         >/dev/null 2>&1 ||
        fail "Firstboot hedef sistemde enable edilemedi."

    echo "✅ Firstboot diskten ilk açılış için enable edildi."
else
    fail "Firstboot service hedef sistemde bulunamadı."
fi

echo "===== CHROOT HAZIRLANIYOR ====="

mount --bind /dev "$MNT/dev"
mount --bind /dev/pts "$MNT/dev/pts"
mount -t proc proc "$MNT/proc"
mount -t sysfs sys "$MNT/sys"
mount --bind /run "$MNT/run"

cp -L /etc/resolv.conf "$MNT/etc/resolv.conf"

echo
echo "===== BENZERSIZ SSH HOST KEY ====="

rm -f "$MNT/etc/ssh/ssh_host_"* 2>/dev/null || true
chroot "$MNT" /usr/bin/ssh-keygen -A
ls -l "$MNT/etc/ssh"/ssh_host_*_key >/dev/null 2>&1 ||
    fail "SSH host key uretilemedi."

echo "SSH host key generation: OK"

echo
echo "===== UPDATE AGENT DOSYA İZİNLERİ ====="

for AGENT_FILE in update_agent.py update_helper.py agent_core.py; do
    if [[ -f "$MNT/opt/photoos/update-agent/$AGENT_FILE" ]]; then
        chown root:root "$MNT/opt/photoos/update-agent/$AGENT_FILE"
        case "$AGENT_FILE" in
            update_agent.py|update_helper.py) chmod 0755 "$MNT/opt/photoos/update-agent/$AGENT_FILE" ;;
            *) chmod 0644 "$MNT/opt/photoos/update-agent/$AGENT_FILE" ;;
        esac
    fi
done

echo "✅ Update Agent ve helper dosya izinleri hazır."

echo
echo "===== INITRAMFS ====="

chroot "$MNT" /bin/bash -c '
set -e
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

update-initramfs -u -k all
'

echo
echo "===== GRUB UEFI ====="

chroot "$MNT" /bin/bash -c '
set -e
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

grub-install \
    --target=x86_64-efi \
    --efi-directory=/boot/efi \
    --bootloader-id=PhotoOS \
    --recheck

update-grub
'

echo
echo "===== GRUB BIOS FALLBACK ====="

chroot "$MNT" /bin/bash -c "
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

grub-install \
    --target=i386-pc \
    --recheck \
    '$TARGET' \
    >/dev/null 2>&1 || true
"

echo
echo "===== PHOTOOS HEDEF KULLANICISI ====="

chroot "$MNT" /bin/bash -c '
set -e

if ! getent group photoos >/dev/null 2>&1; then
    groupadd --system photoos
fi

if ! id photoos >/dev/null 2>&1; then
    useradd --create-home --shell /bin/bash --gid photoos photoos
fi

for GROUP in sudo disk docker; do
    if getent group "$GROUP" >/dev/null 2>&1; then
        usermod -aG "$GROUP" photoos || true
    fi
done
'

echo "✅ Hedef sistemde photoos kullanıcısı hazır."

echo
echo "===== INSTALLER 3.1 SİSTEM AYARLARI ====="

if [[ "$PHOTOOS_INSTALLER3" == "1" ]]; then

    [[ "$PHOTOOS_HOSTNAME" =~ ^[a-zA-Z0-9][a-zA-Z0-9.-]{0,62}$ ]] ||
        fail "Geçersiz hostname."

    printf '%s\n' "$PHOTOOS_HOSTNAME" > "$MNT/etc/hostname"

    # localhost satırlarını koruyarak hostname eşlemesi ekle.
    if ! grep -qE '^127\.0\.1\.1[[:space:]]+' "$MNT/etc/hosts"; then
        printf '127.0.1.1\t%s\n' "$PHOTOOS_HOSTNAME" \
            >> "$MNT/etc/hosts"
    else
        sed -i -E \
            "s/^127\.0\.1\.1[[:space:]].*/127.0.1.1\t$PHOTOOS_HOSTNAME/" \
            "$MNT/etc/hosts"
    fi

    if [[ -e "$MNT/usr/share/zoneinfo/$PHOTOOS_TIMEZONE" ]]; then
        ln -sfn \
            "/usr/share/zoneinfo/$PHOTOOS_TIMEZONE" \
            "$MNT/etc/localtime"

        printf '%s\n' "$PHOTOOS_TIMEZONE" \
            > "$MNT/etc/timezone"
    else
        fail "Geçersiz saat dilimi: $PHOTOOS_TIMEZONE"
    fi

    mkdir -p "$MNT/etc/photoos"

    python3 - \
        "$PHOTOOS_SERVER_NAME" \
        "$PHOTOOS_HOSTNAME" \
        "$PHOTOOS_TIMEZONE" \
        "$TARGET" \
        > "$MNT/etc/photoos/install.json" <<'PYINSTALL'
import json
import sys

print(json.dumps({
    "product": "PhotoOS",
    "version": "1.2.1-rc2",
    "installer_version": "3.1",
    "server_name": sys.argv[1],
    "hostname": sys.argv[2],
    "timezone": sys.argv[3],
    "target_disk": sys.argv[4],
    "ssh": {
        "enabled": True,
        "user": "photoos"
    }
}, indent=2, ensure_ascii=False))
PYINSTALL

    chmod 0644 "$MNT/etc/photoos/install.json"

    [[ -n "$PHOTOOS_SSH_PASSWORD_FILE" ]] ||
        fail "SSH parola dosyası belirtilmedi."

    [[ -f "$PHOTOOS_SSH_PASSWORD_FILE" ]] ||
        fail "SSH parola dosyası bulunamadı."

    MODE="$(stat -c '%a' "$PHOTOOS_SSH_PASSWORD_FILE")"

    [[ "$MODE" == "600" ]] ||
        fail "SSH parola dosyasının izni 600 olmalı (mevcut: $MODE)."

    SSH_PASSWORD="$(cat "$PHOTOOS_SSH_PASSWORD_FILE")"

    [[ -n "$SSH_PASSWORD" ]] ||
        fail "SSH parolası boş."

    printf 'photoos:%s\n' "$SSH_PASSWORD" |
        chroot "$MNT" /usr/sbin/chpasswd

    unset SSH_PASSWORD

    echo "✅ Hostname: $PHOTOOS_HOSTNAME"
    echo "✅ Saat dilimi: $PHOTOOS_TIMEZONE"
    echo "✅ photoos SSH parolası ayarlandı."
fi


echo
echo "===== PHOTOOS DİZİNLERİ ====="

chown -R photoos:photoos \
    "$MNT/var/lib/photoos" \
    "$MNT/srv/photoos" \
    "$MNT/srv/luminos" \
    2>/dev/null || true

echo
echo "===== PHOTOOS UPDATE AGENT TOKEN ====="

mkdir -p "$MNT/etc/photoos"
TOKEN_ENV_FILE="$MNT/etc/photoos/update-agent.env"

if [[ -f "$TOKEN_ENV_FILE" ]]; then
    grep -Eq '^PHOTOOS_UPDATE_TOKEN=[0-9a-f]{64}$' "$TOKEN_ENV_FILE" ||
        fail "Mevcut Update Agent token dosyası geçersiz."
else
    TOKEN="$(
        chroot "$MNT" /usr/bin/python3 - <<'PYTOKEN'
import secrets
print(secrets.token_hex(32))
PYTOKEN
    )"
    umask 0077
    printf 'PHOTOOS_UPDATE_TOKEN=%s\n' "$TOKEN" > "$TOKEN_ENV_FILE"
    unset TOKEN
fi

chmod 0640 "$TOKEN_ENV_FILE"
chown root:photoos "$TOKEN_ENV_FILE"

echo "✅ Update Agent token hazır."

echo
echo "===== SSH ====="

chroot "$MNT" systemctl enable ssh.service \
    >/dev/null 2>&1 || true

echo
echo "===== PHOTOOS ====="

if [[ -f "$MNT/etc/systemd/system/photoos.service" ]]; then
    chroot "$MNT" systemctl enable photoos.service \
        >/dev/null 2>&1 || true
fi

echo
echo "===== PHOTOOS NOTIFICATION SERVİSLERİ ====="

for UNIT in \
    photoos-storage-notification-producer.timer \
    photoos-notification-aggregator.timer
do
    if [[ -e "$MNT/etc/systemd/system/$UNIT" ]]; then
        chroot "$MNT" systemctl enable "$UNIT" \
            >/dev/null 2>&1 || true

        echo "✅ Enable: $UNIT"
    else
        echo "⚠️ Hedef sistemde bulunamadı: $UNIT"
    fi
done

echo
echo "===== PHOTOOS UPDATE SERVİSLERİ ====="

for UNIT in photoos-update-helper.service photoos-update-agent.service; do
    if [[ -f "$MNT/etc/systemd/system/$UNIT" ]]; then
        chroot "$MNT" systemctl enable "$UNIT" >/dev/null 2>&1 || true
        echo "✅ Enable: $UNIT"
    else
        echo "⚠️ Hedef sistemde bulunamadı: $UNIT"
    fi
done

echo
echo "===== FINAL DETERMINISTIC SERVICE ENABLE ====="

REQUIRED_UNITS=(
    ssh.service
    systemd-timesyncd.service
    photoos.service
    photoos-storage-helper.service
    photoos-storage-core.service
    photoos-setup-provisioner.service
    photoos-storage-notification-producer.timer
    photoos-notification-aggregator.timer
    photoos-disk-health.timer
    photoos-update-helper.service
    photoos-update-agent.service
    photoos-firstboot-installer.service
)

for UNIT in "${REQUIRED_UNITS[@]}"; do
    if [[ ! -e "$MNT/etc/systemd/system/$UNIT" &&
          ! -e "$MNT/lib/systemd/system/$UNIT" &&
          ! -e "$MNT/usr/lib/systemd/system/$UNIT" ]]; then
        fail "Zorunlu systemd unit hedef sistemde yok: $UNIT"
    fi

    systemctl --root="$MNT" enable "$UNIT" >/dev/null ||
        fail "Zorunlu systemd unit enable edilemedi: $UNIT"

    echo "OK enable: $UNIT"
done

echo "Tum zorunlu servis/timer enable kontrolleri gecti."

echo
echo "=================================================="
echo " ✅ PHOTOOS DİSKE KURULDU"
echo "=================================================="
echo
echo "Hedef disk : $TARGET"
echo "Root       : $ROOT"
echo "EFI        : $EFI"
echo
echo "Şimdi:"
echo "1. sistemi kapatın"
echo "2. USB belleği çıkarın"
echo "3. cihazı yeniden açın"
