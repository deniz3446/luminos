# PhotoOS Fotoğraf Kütüphanesi Yenileme Tasarımı

## Amaç

Mevcut Fotoğraf Arşivi ekranını, kullanıcının sağladığı hedef görsele yakın, profesyonel lacivert-mavi bir PhotoOS fotoğraf merkezine dönüştürmek. Tasarım görsel olarak yenilenirken mevcut gerçek veriler, API sözleşmeleri ve fotoğraf yönetimi işlevleri korunacaktır.

## Kapsam

Bu çalışma aşağıdaki alanları kapsar:

- Fotoğraflar sayfasının üst başlık, istatistik, filtre, zaman ve galeri alanlarının yeniden tasarlanması.
- Ana uygulama kabuğunun (sol menü ve üst çubuk) hedef görsel ile uyumlu hale getirilmesi.
- Fotoğrafların `source_type` değerine göre ayrıştırılması.
- Masaüstü, tablet ve telefon boyutları için responsive düzen.
- Mevcut fotoğraf görüntüleyicisi ve yönetim işlemlerinin korunması.

Bu çalışma fotoğraf dosyalarını, veritabanındaki mevcut kaynak değerlerini veya kullanıcı verilerini topluca değiştirmez.

## Bilinen Canlı Veri

20 Eylül 2026 tarihli salt-okunur veritabanı sorgusunda kaynak dağılımı şöyledir:

- `whatsapp_received`: 540
- `camera`: 105
- `screenshot`: 41
- `download`: 1
- `other`: 1

Mevcut tabloda `source_type`, `source_path`, `device_name` ve `device_id` alanları bulunmaktadır. `pc_backup` türünde mevcut kayıt yoktur.

## Kaynak Sınıflandırması

Arayüz şu kaynakları ayrı filtreler olarak gösterecektir:

1. Tümü
2. Kamera (`camera`)
3. WhatsApp Gelen (`whatsapp_received`)
4. WhatsApp Gönderilen (`whatsapp_sent`)
5. Ekran Görüntüleri (`screenshot`)
6. İndirilenler (`download`)
7. Telegram (`telegram`)
8. PC Backup (`pc_backup`)
9. Diğer (`other` ve tanınmayan değerler)

Sınıflandırma dosya adına veya dizin adına bakılarak tahmin edilmeyecektir. Arayüz yalnızca API'nin gönderdiği `source_type` değerini kullanacaktır. PC Backup istemcisi ileride `source_type=pc_backup` gönderdiğinde ilgili filtre otomatik olarak çalışacaktır. Mevcut kayıtlar geriye dönük olarak değiştirilmez.

## Arayüz Yapısı

### Uygulama kabuğu

- Sol menü koyu lacivert arka plan, mavi aktif durum ve okunaklı ikonlarla yenilenecektir.
- PhotoOS markası, sunucu durumu ve kullanıcı bölümü görsel hiyerarşiye uygun yerleştirilecektir.
- Mevcut rota bağlantıları ve rol kontrolleri korunacaktır.
- Mobil görünümde menü açılır-kapanır olacaktır; Escape ve rota değişimi menüyü kapatacaktır.

### Fotoğraflar üst alanı

- Sayfa başlığı ve kısa güven mesajı gösterilecektir.
- Metin araması, yüklü fotoğraflar içinde `original_name`, `filename`, kullanıcı, cihaz ve kaynak etiketi üzerinden istemci tarafında çalışacaktır.
- Yenileme, seçim modu, medya türü ve kullanıcı filtreleri korunacaktır.
- Yükleme işlevi mevcut kaynakta bağlı değilse çalışıyormuş gibi görünen sahte bir düğme eklenmeyecektir.

### İstatistik kartları

- Fotoğraf sayısı
- Video sayısı
- Albüm sayısı
- Görüntülenen sonuçların tarih aralığı

Sayılar yalnızca yüklenen veri ve mevcut API yanıtlarından üretilecektir. Sunucudaki tüm kayıtları temsil etmeyen sayılar açıkça mevcut yüklenen küme bağlamında gösterilecek veya toplam endpoint kullanılacaktır.

### Filtre ve zaman şeridi

- Kaynak filtreleri yatay, kaydırılabilir düğmeler olarak sunulacaktır.
- Yıl ve ay seçimleri büyük kapak kartları yerine kompakt sekmeler olacaktır.
- Aktif filtreler belirgin mavi vurgu ile gösterilecektir.
- Filtre kombinasyonları kaynak, medya, kullanıcı, arama, yıl ve ay sırasıyla uygulanacaktır.

### Galeri

- Fotoğraflar hedef görseldeki gibi sık ve dengeli bir ızgarada gösterilecektir.
- Görsel oranları korunurken kart alanı `object-fit: cover` kullanacaktır.
- Video ve kaynak türü işaretleri kart üzerinde erişilebilir etiketlerle bulunacaktır.
- Seçim modu, seçili kart durumu ve dosya işlemleri korunacaktır.
- Artan veri için mevcut görünür limit ve “daha fazla göster” davranışı korunacaktır.

### Görüntüleyici

- Mevcut tam ekran görüntüleyici, önceki/sonraki gezinme, Escape ile kapanma, detay bilgisi ve silme işlemi korunacaktır.
- Yenileme görüntüleyicinin mevcut davranışını değiştirmeyecektir.

## Durum ve Veri Akışı

1. Sayfa `/api/v1/photos` ve `/api/v1/photos/count` üzerinden veriyi yükler.
2. Sunucu tarafında desteklenen kaynak ve medya filtreleri sorguya eklenir.
3. Kullanıcı, arama, yıl ve ay gibi yüklü veri üzerinde çalışan filtreler türetilmiş değer olarak hesaplanır.
4. Kaynak değiştiğinde sonuçlar sıfırlanır ve ilk sayfa yeniden yüklenir.
5. Seçim listesi yalnızca görünür filtre değil, benzersiz dosya adı üzerinden korunur ve açık temizleme işlemleriyle sıfırlanır.

## Hata ve Boş Durumlar

- API hatası sayfa içinde anlaşılır bir uyarı ve yeniden deneme seçeneği göstermelidir.
- Sonuç bulunmadığında aktif filtreleri açıklayan boş durum gösterilmelidir.
- Sıfır kayıtlı kaynak filtresi kaybolmamalı; `0` durumuyla seçilebilir kalmalıdır.
- Bozuk küçük resim tek kartı etkilemeli, tüm galeriyi bozmamalıdır.
- PC Backup verisi yokken başka kaynaklar PC Backup olarak etiketlenmemelidir.

## Erişilebilirlik

- Tüm ikon düğmelerinde erişilebilir ad bulunacaktır.
- Aktif filtrelerde `aria-pressed` veya eşdeğer semantik durum kullanılacaktır.
- Klavye odağı görünür olacaktır.
- Renk tek başına seçim göstergesi olmayacaktır.
- Mobil menüde `aria-expanded` ve `aria-controls` bulunacaktır.

## Dosya Sınırları

Başlıca değişiklikler:

- `src/pages/PhotosPage.tsx`: veri akışı ve yeni sayfa bileşimi.
- `src/pages/PhotosPage.css`: Fotoğraflar sayfasına özel stil ve responsive kurallar.
- `src/layouts/DashboardLayout.tsx`: modern kabuk ve mobil menü davranışı.
- `src/index.css`: ortak tema değişkenleri ve kabuk stilleri.

Gerekirse küçük, yalnızca sunuma yönelik bileşenler `src/components/photos/` altında ayrıştırılacaktır. API istemcisi, işlev gerektirmedikçe değiştirilmez.

## Doğrulama

- TypeScript üretim derlemesi başarılı olmalıdır.
- ESLint sonucu kaydedilmeli; yeni hata eklenmemelidir.
- Fotoğraf listeleme ve toplam endpointleri HTTP 200 dönmelidir.
- Kamera, WhatsApp Gelen, Ekran Görüntüleri ve Diğer filtreleri canlı veride doğrulanmalıdır.
- Sıfır sonuçlu PC Backup filtresi doğru boş durum göstermelidir.
- Seçim, ZIP indirme, albüme ekleme, görüntüleyici gezinmesi ve silme akışları kontrol edilmelidir.
- Masaüstü, tablet ve telefon genişliklerinde taşma ve kullanılabilirlik kontrol edilmelidir.

## Güvenli Yayın

1. Değişiklikler kurtarılmış kaynak kopyasında uygulanır.
2. Ayrı bir önizleme derlemesi oluşturulur; aktif sürüme yazılmaz.
3. Derleme ve smoke kontrolleri tamamlanır.
4. Mevcut `/opt/photoos/current` hedefi ve aktif sürüm korunur.
5. Yeni sürüm ayrı bir release dizinine kurulur.
6. Yalnızca doğrulama başarılıysa `current` bağlantısı kontrollü biçimde değiştirilir ve servis yeniden başlatılır.
7. Sağlık kontrolü başarısız olursa önceki release hedefine geri dönülür.

## Başarı Ölçütleri

- Fotoğraflar sayfası hedef görselin profesyonel lacivert-mavi karakterine belirgin biçimde yaklaşır.
- Gerçek fotoğraflar ve mevcut yönetim işlevleri çalışmaya devam eder.
- Kaynaklar gerçek `source_type` değerleriyle ayrılır.
- PC Backup, veri geldiğinde başka kod değişikliği olmadan ayrı filtrede görünür.
- Aktif PhotoOS sürümü başarısız veya doğrulanmamış bir derlemeyle değiştirilmez.
