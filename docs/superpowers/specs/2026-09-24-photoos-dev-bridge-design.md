# PhotoOS Dev Bridge Design

## Amac
Kullanici yalnizca istedigi PhotoOS degisikligini dogal dille belirtir. ChatGPT, Remote Desktop Commander uzerinden PC'deki canonical kaynak agacini duzenler, test eder, commit olusturur ve GitHub'a push eder.

## Temel akis
1. Istek alinir ve ilgili PhotoOS kaynak kodu incelenir.
2. Her is icin `ai/<kisa-gorev>` branch'i acilir.
3. Degisiklikler canonical calisma agacinda yapilir.
4. Targeted testler, ardindan ilgili build/test kapilari calistirilir.
5. Test basarisizsa push yapilmaz; hata duzeltilir ve test tekrarlanir.
6. Testler basariliysa commit olusturulur ve GitHub'a push edilir.
7. Basarili degisiklik `main` ile birlestirilir ve kullaniciya kisa sonuc raporu verilir.

## Repo rolleri
- `deniz3446/luminos`: PhotoOS ana kaynak deposu ve `main` canonical kaynak.
- `deniz3446/PhotoOS-Updates`: yalniz imzali update dagitimi; normal kod gelistirmede degistirilmez.
- PC canonical klasoru: `C:\Users\MSI\PhotoOS-Dev`.
- `C:\Users\MSI\PhotoOS-AI`: ajan kurallari, helper scriptleri ve bu tasarim.

## Ilk esitleme
GitHub'daki eski `luminos/main` korunmadan ezilmez. Once mevcut `main`, `archive/luminos-legacy-20260628` branch'i olarak korunur. Ardindan sunucudaki dogrulanmis guncel PhotoOS kaynak tarihi PC'ye alinip yeni canonical `main` olarak GitHub'a gonderilir.

## Test mimarisi
- Frontend: PC'de Node/npm ile lint, test ve production build.
- Backend: Windows'ta Rust toolchain hazir degilse PhotoOS sunucusunda canli release'den tamamen ayri bir test worktree kullanilir.
- Backend test worktree hicbir zaman `/opt/photoos/current`, runtime DB, storage diskleri veya systemd servislerini degistirmez.
- Gerekirse daha sonra Rust Windows toolchain kurulabilir; bu bootstrap icin zorunlu degildir.

## Guvenlik sinirlari
- Normal gelistirme akisi kaynak kodu + test + Git ile sinirlidir.
- Canli PhotoOS deploy, servis restart, update install, reboot, network/firewall, kullanici/izin ve disk islemleri ayri kullanici onayi gerektirir.
- `PHOTOOS_DATA1` ve `PHOTOOS_DATA2` disklerine destructive islem yasaktir.
- Secret, JWT, update token veya signing private key loglara ya da Git'e yazilmaz.
- Production signing ve update publication normal kod push akisinin parcasi degildir.

## Git davranisi
- Her gorev izole branch'te baslar.
- Mevcut kirli calisma agaci varsa otomatik ezilmez; once raporlanir ve izole worktree kullanilir.
- Test kapilari gecmeden commit/push yapilmaz.
- Force push varsayilan olarak yasaktir.
- `main`e birlestirme sadece testler basarili ve branch beklenen base uzerindeyse fast-forward veya kontrollu merge ile yapilir.

## Basari kriteri
Kurulumdan sonra kullanici `Fotograflar sayfasinda X'i duzelt` gibi tek bir istek verir. ChatGPT PC'ye baglanir, kodu degistirir, testleri calistirir, GitHub'a gonderir ve yalniz sonuc/degisen dosyalar/test durumunu raporlar. Kullanici normal gelistirmede PowerShell/Bash komutu calistirmaz.
