# PhotoOS AI / Dev Bridge Kurallari

## Hedef
PhotoOS kaynak kodunun geliştirilmesi, test edilmesi ve GitHub'a güvenli şekilde gönderilmesi. Normal geliştirme akışı canlı PhotoOS sistemini değiştirmez.

## Bağlantı
- PC işlemlerinde Remote Desktop Commander kullan.
- PhotoOS sunucusuna gerektiğinde SSH aliası `photoos-server` veya mevcut doğrulanmış Paramiko bağlantısı ile bağlan.
- IP adresini ürün koduna sabitleme.
- Önce mevcut sistemi/kaynağı incele, sonra değişiklik yap.

## Canonical kaynak
- PC canonical repo: `C:\Users\MSI\PhotoOS-Dev`.
- Ana GitHub repo: `deniz3446/luminos`.
- Update dağıtım repo: `deniz3446/PhotoOS-Updates`; normal kod geliştirmede değiştirme.
- Her iş `ai/<kisa-gorev>` branch'inde yapılır.

## Güvenlik
- Her zaman önce salt-okunur teşhis yap.
- `PHOTOOS_DATA1` ve `PHOTOOS_DATA2` veri disklerine format, partition, wipe veya destructive yazma yapma.
- `dd`, `mkfs`, `wipefs`, `fdisk`, `parted`, `pvcreate`, `mdadm --create` açık kullanıcı onayı olmadan yasaktır.
- Reboot, shutdown, firewall, network, kullanıcı/izin, disk, update install, production signing ve canlı deploy/restart için ayrıca kullanıcı onayı iste.

## Geliştirme akışı
- Kaynak kodu incelemeden değişiklik yapma.
- Bug düzeltmelerinde önce problemi yeniden üret.
- Mümkünse önce test yaz ve RED sonucu gör.
- Değişiklikten sonra targeted testleri, ardından ilgili tam test/build kapılarını çalıştır.
- Test başarısızsa commit/push yapma; hatayı düzelt ve tekrar test et.
- Testler başarılıysa normal sonuç: commit oluştur, task branch'ini GitHub'a push et ve uygun olduğunda test edilmiş değişikliği `main`e birleştir/push et.
- Force push yapma.
- Canlı PhotoOS dosyalarını doğrudan düzenleme.
- Canlı deploy, service restart veya update install hiçbir zaman normal Git akışının otomatik devamı değildir; ayrıca kullanıcı onayı gerekir.

## Test politikası
- Frontend PC üzerinde `npm.cmd` ile çalıştırılır.
- En az `npm.cmd run contract-check` ve `npm.cmd run build` geçmelidir.
- Lint'te mevcut baseline borç varsa sayı ve dosyalar raporlanır; yeni regresyon gizlenmez.
- Rust backend testleri Windows toolchain yokken sunucuda `/home/photoos/PhotoOS-worktrees/ai-*` altında izole worktree'de çalıştırılır.
- Backend test worktree'si `/opt/photoos/current`, runtime DB, storage diskleri veya systemd servislerini değiştirmez.

## Raporlama
- Ne bulunduğunu kısa ve net anlat.
- Değişen dosyaları ve commit SHA'yı belirt.
- Çalıştırılan testleri ve sonuçlarını belirt.
- GitHub branch/main durumunu belirt.
- Riskli canlı işleme geçmeden önce ayrıca onay iste.
