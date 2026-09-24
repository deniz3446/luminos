# PhotoOS Dev Bridge

PhotoOS Dev Bridge, ChatGPT'nin kullanıcının Windows PC'sindeki canonical PhotoOS kaynak koduna Remote Desktop Commander üzerinden erişip güvenli geliştirme yapması için kullanılan yardımcı katmandır.

## Kullanıcı akışı
Kullanıcı yalnızca istediği değişikliği doğal dille belirtir. Normal geliştirme akışı: incele → task branch → değişiklik → test → commit → GitHub push → rapor.

Kullanıcıdan normal geliştirme sırasında PowerShell/Bash komutu çalıştırması beklenmez.

## PC yolları
- Canonical repo: `C:\Users\MSI\PhotoOS-Dev`
- Çalışan yardımcı kopya: `C:\Users\MSI\PhotoOS-AI`
- Masaüstü kısayolu: `PhotoOS Dev Bridge.lnk`

## Test kapıları
Frontend için en az `npm.cmd run contract-check` ve `npm.cmd run build` geçer. Backend Rust kontrolleri Windows toolchain yokken PhotoOS sunucusunda canlı release'den ayrı `/home/photoos/PhotoOS-worktrees/ai-*` worktree'sinde çalışır.

## Güvenlik sınırı
Normal akış canlı PhotoOS deploy etmez. `/opt/photoos/current`, systemd servisleri, update install, production signing, network/firewall, reboot ve disk işlemleri ayrı kullanıcı onayı gerektirir.

Force push kullanılmaz. Secret, JWT, update token ve private signing key Git'e veya rapora yazılmaz.

## Remote Commander
Masaüstündeki **PhotoOS Dev Bridge** kısayolu durum uygulamasını açar. Uygulamadaki **Remote Commander Başlat** düğmesi şu güvenli istemci komutunu çalıştırır:

`npx.cmd @wonderwhy-er/desktop-commander@latest remote`

Bağlantı penceresi açık kaldığı sürece ChatGPT yetkili PC üzerinde dosya/terminal işlemlerini gerçekleştirebilir.
