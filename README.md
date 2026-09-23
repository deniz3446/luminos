# PhotoOS Installer Kit

Bu ilk installer sürümü:

- Mevcut `/opt/photoos/releases/<sürüm>` içeriğini paketler.
- `/opt/photoos/update-agent` içeriğini paketler.
- Tek dosyalık `PhotoOS-Installer-<sürüm>.run` üretir.
- Hedef Debian makinede dizinleri, servisleri ve release symlink'ini kurar.
- Takılı diskleri listeler fakat hiçbir diski biçimlendirmez.

## Kaynak PhotoOS makinesinde

Kit içindeki `installer` klasörünü `~/PhotoOS/installer` altına kopyalayın.

Ardından:

```bash
cd ~/PhotoOS
chmod +x installer/*.sh
sudo -v
./installer/build-installer.sh "$HOME/PhotoOS" 1.0.0
```

Çıktı:

```text
~/PhotoOS/dist/PhotoOS-Installer-1.0.0.run
```

## Dosyayı hedef makineye taşıma

USB bellek, SCP veya yerel ağ kullanılabilir.

Örnek:

```bash
scp ~/PhotoOS/dist/PhotoOS-Installer-1.0.0.run kullanici@HEDEF_IP:/tmp/
```

## Hedef Debian makinede

```bash
chmod +x /tmp/PhotoOS-Installer-1.0.0.run
sudo /tmp/PhotoOS-Installer-1.0.0.run
```

Kurulum günlüğü:

```text
/var/log/photoos-installer.log
```

## Önemli

Bu sürüm storage diski seçmez, biçimlendirmez ve RAID kurmaz. Disk yönetimi sonraki installer sürümünde güvenli onay adımlarıyla eklenecektir.
