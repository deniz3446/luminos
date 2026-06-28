# PhotoOS Roadmap

PhotoOS, ev kullanıcıları ve küçük işletmeler için geliştirilen kişisel fotoğraf ve video yedekleme sistemidir.

## Hedef Kullanıcı

- Ev kullanıcıları
- Aileler
- Fotoğrafçılar
- Küçük işletmeler

Hedef ölçek:

- 1–50 kullanıcı
- Yüz binlerce / milyonlarca fotoğraf
- Tek sunucu / NAS odaklı kullanım

---

## Sprint 1 - Temel Backend

- [x] Rust workspace
- [x] Axum HTTP server
- [x] Config sistemi
- [x] SQLite bağlantısı
- [x] SQLx migration
- [x] Kullanıcı oluşturma
- [x] Argon2 şifre hashleme
- [x] Kullanıcı listeleme
- [x] Password hash gizleme
- [x] Login
- [x] JWT üretme
- [x] CORS desteği

---

## Sprint 2 - Web Arayüz

- [x] React + TypeScript
- [x] Vite
- [x] Login ekranı
- [x] Dashboard
- [x] Kullanıcı listesi
- [x] Siyah-kırmızı tema
- [x] Sidebar layout

---

## Sprint 3 - Fotoğraf Yükleme

- [ ] Photo veri modeli
- [ ] Storage dizin yapısı
- [ ] Upload endpoint
- [ ] Dosyayı diske kaydetme
- [ ] Fotoğraf kaydını SQLite'a yazma
- [ ] Frontend upload ekranı
- [ ] Fotoğraf listeleme

---

## Sprint 4 - Medya İşleme

- [ ] Thumbnail üretme
- [ ] Video önizleme
- [ ] EXIF metadata okuma
- [ ] SHA256 duplicate kontrolü
- [ ] Tarihe göre gruplama

---

## Sprint 5 - Mobil Yedekleme Hazırlığı

- [ ] Mobil upload API
- [ ] Cihaz kaydı
- [ ] Otomatik yedekleme kuyruğu
- [ ] Wi-Fi upload modu
- [ ] Yedeklenen dosyayı telefondan silme mantığı

---

## Sprint 6 - Albümler

- [ ] Albüm oluşturma
- [ ] Albüme fotoğraf ekleme
- [ ] Albümden fotoğraf çıkarma
- [ ] Albüm paylaşımı

---

## Uzun Vadeli Hedefler

- Android uygulaması
- iOS uygulaması
- Windows sync client
- macOS sync client
- Docker kurulumu
- Web installer
- Plugin sistemi
- AI arama
- Yüz tanıma
- Nesne tanıma
- Çoklu disk desteği
- Backup sistemi
