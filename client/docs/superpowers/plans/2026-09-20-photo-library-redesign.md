# PhotoOS Fotoğraf Kütüphanesi Yenileme Uygulama Planı

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** PhotoOS Fotoğraflar ekranını gerçek kaynak ayrımını koruyan, profesyonel lacivert-mavi, responsive bir fotoğraf merkezine dönüştürmek.

**Architecture:** Mevcut API ve fotoğraf işlem akışları korunacak; görünüm `PhotosPage.tsx` ile yeni sayfaya özel CSS üzerinden yenilenecek. Kaynak tanımları ve saf filtreleme yardımcıları ayrı bir modüle alınacak, uygulama kabuğu `DashboardLayout.tsx` ve ortak tema CSS'i ile modernleştirilecek. Yeni derleme önce izole önizleme dizininde doğrulanacak, ardından geri dönüş noktası korunarak ayrı release olarak yayınlanacak.

**Tech Stack:** React 19, TypeScript 6, React Router 7, Vite 8, CSS, Node.js, Rust/Axum PhotoOS API, SQLite.

**Spec:** `docs/superpowers/specs/2026-09-20-photo-library-redesign-design.md`

## Global Constraints

- Mevcut fotoğraf dosyaları ve veritabanı kayıtları topluca değiştirilmeyecek.
- Kaynak sınıflandırması yalnızca API'nin `source_type` alanına dayanacak; dosya veya dizin adından tahmin yapılmayacak.
- Desteklenen ayrıntılı kaynaklar: `camera`, `whatsapp_received`, `whatsapp_sent`, `screenshot`, `download`, `telegram`, `pc_backup`, `other`.
- Sıfır kayıtlı kaynaklar gizlenmeyecek.
- ZIP indirme, albüme ekleme, silme, seçim ve görüntüleyici davranışları korunacak.
- Sahte veya bağlı olmayan bir yükleme düğmesi eklenmeyecek.
- Aktif `/opt/photoos/current` doğrulanmamış bir derlemeye yönlendirilmeyecek.
- Yeni runtime bağımlılığı eklenmeyecek.

## Review Focus

- Bilinmeyen veya boş `source_type`, PC Backup sayılmadan “Diğer” olarak gösterilmeli.
- Aynı anda kaynak, kullanıcı, medya, arama, yıl ve ay filtresi uygulandığında sonuçların kesişimi gösterilmeli.
- Boş sonuç, yükleme hatası ve kırık küçük resim sayfanın kalanını bozmamalı.
- Mobil menü rota değişiminde ve Escape tuşunda kapanmalı; masaüstünde sürekli görünmeli.
- Yüklü ilk 500 kayıt toplam sunucu sayısından azsa istatistik etiketi kullanıcıyı yanıltmamalı.

---

## Dosya Haritası

- `src/pages/photoLibraryModel.ts`: Kaynak kataloğu, tarih/arama normalizasyonu ve saf filtreleme yardımcıları.
- `src/pages/photoLibraryModel.test.mjs`: Model sözleşmesini Node test runner ile sabitleyen testler.
- `src/pages/PhotosPage.tsx`: Veri yükleme, filtre durumu, galeri, seçim işlemleri ve görüntüleyici.
- `src/pages/PhotosPage.css`: Fotoğraf sayfasına özel bileşen ve responsive stiller.
- `src/layouts/DashboardLayout.tsx`: Modern uygulama kabuğu ve mobil menü davranışı.
- `src/index.css`: Tema değişkenleri, kabuk, sol menü ve üst çubuk stilleri.

### Task 1: Kaynak kataloğu ve filtreleme modeli

**Files:**
- Create: `src/pages/photoLibraryModel.ts`
- Create: `src/pages/photoLibraryModel.test.mjs`

**Interfaces:**
- Consumes: Fotoğraf nesnesinin `filename`, `original_name`, `owner_username`, `source_type`, `device_name`, `mime_type`, `taken_at`, `uploaded_at` alanları.
- Produces: `SOURCE_OPTIONS`, `normalizeSourceType(value)`, `getPhotoDate(photo)`, `matchesPhotoSearch(photo, query)` ve `filterPhotoCollection(photos, filters)`.

- [ ] **Step 1: Kaynak sözleşmesi için başarısız testi yaz**

`src/pages/photoLibraryModel.test.mjs` içinde dosya metnini okuyarak sekiz gerçek kaynak değerinin, PC Backup'ın ve bilinmeyen değerlerin `other` sonucunun tanımlı olduğunu doğrula:

```js
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const source = readFileSync(new URL("./photoLibraryModel.ts", import.meta.url), "utf8");

test("ayrıntılı kaynak kataloğunu tanımlar", () => {
  for (const value of ["camera", "whatsapp_received", "whatsapp_sent", "screenshot", "download", "telegram", "pc_backup", "other"]) {
    assert.match(source, new RegExp(`value:\\s*[\"']${value}[\"']`));
  }
});

test("bilinmeyen kaynakları other olarak normalize eder", () => {
  assert.match(source, /return\s+[\"']other[\"']/);
});
```

- [ ] **Step 2: Testin doğru nedenle başarısız olduğunu doğrula**

Run: `node --test src/pages/photoLibraryModel.test.mjs`

Expected: FAIL; `photoLibraryModel.ts` bulunamadığı için `ENOENT`.

- [ ] **Step 3: Saf model modülünü oluştur**

`photoLibraryModel.ts` içinde `PhotoLike` ve `PhotoFilters` arayüzlerini tanımla. `normalizeSourceType` yalnızca katalogdaki değerleri döndürsün, diğer tüm değerlerde `other` dönsün. `matchesPhotoSearch` Türkçe locale ile küçük harfe dönüştürülmüş sorguyu dosya adı, özgün ad, kullanıcı, cihaz ve kaynak etiketi üzerinde arasın. `filterPhotoCollection` tüm aktif filtrelerin kesişimini döndürsün.

```ts
export const SOURCE_OPTIONS = [
  { value: "all", label: "Tümü", icon: "▦" },
  { value: "camera", label: "Kamera", icon: "▣" },
  { value: "whatsapp_received", label: "WhatsApp Gelen", icon: "◉" },
  { value: "whatsapp_sent", label: "WhatsApp Gönderilen", icon: "↗" },
  { value: "screenshot", label: "Ekran Görüntüleri", icon: "▤" },
  { value: "download", label: "İndirilenler", icon: "↓" },
  { value: "telegram", label: "Telegram", icon: "➤" },
  { value: "pc_backup", label: "PC Backup", icon: "▰" },
  { value: "other", label: "Diğer", icon: "◇" },
] as const;
```

- [ ] **Step 4: Model testini ve TypeScript kontrolünü çalıştır**

Run: `node --test src/pages/photoLibraryModel.test.mjs`

Expected: PASS, 2 tests.

Run: `npm run build`

Expected: PASS; model modülünde TypeScript hatası yok.

- [ ] **Step 5: Değişikliği commit et**

```bash
git add src/pages/photoLibraryModel.ts src/pages/photoLibraryModel.test.mjs
git commit -m "feat(photos): add source-aware library model"
```

### Task 2: Modern ve responsive uygulama kabuğu

**Files:**
- Modify: `src/layouts/DashboardLayout.tsx`
- Modify: `src/index.css`

**Interfaces:**
- Consumes: `location.pathname`, mevcut kullanıcı adı ve rol bilgisi.
- Produces: `#photoos-sidebar`, `.mobile-menu-toggle`, `.sidebar-backdrop`, `.topbar-search` ve mevcut rotaları koruyan navigasyon.

- [ ] **Step 1: Kabuk sözleşmesi için başarısız statik test ekle**

`photoLibraryModel.test.mjs` içine `DashboardLayout.tsx` metnini okuyup `id="photoos-sidebar"`, `aria-expanded`, `aria-controls="photoos-sidebar"`, `Menüyü aç` ve `Escape` ifadelerini doğrulayan test ekle.

- [ ] **Step 2: Testin başarısız olduğunu doğrula**

Run: `node --test src/pages/photoLibraryModel.test.mjs`

Expected: FAIL; yeni mobil menü sözleşmesi mevcut değil.

- [ ] **Step 3: Kabuk davranışını uygula**

`DashboardLayout.tsx` içinde `mobileMenuOpen` durumu ekle. Mobil düğme açıp kapatsın; `location.pathname` değiştiğinde ve Escape basıldığında kapansın. Sidebar'a `id="photoos-sidebar"`, düğmeye dinamik erişilebilir ad, `aria-expanded` ve `aria-controls` ekle. Mevcut rol bazlı rotaları değiştirme.

- [ ] **Step 4: Lacivert-mavi kabuk stillerini uygula**

`index.css` tema değişkenlerini `--bg-main: #06101f`, `--bg-sidebar: #07172a`, `--accent: #1597ff`, `--text-muted: #91a6c2` ekseninde güncelle. 760px altında sidebar'ı ekran dışına taşı, `.mobile-open` ile göster ve backdrop ekle. 761px üstünde mobil düğmeyi gizle.

- [ ] **Step 5: Kabuk testini ve build'i çalıştır**

Run: `node --test src/pages/photoLibraryModel.test.mjs`

Expected: PASS.

Run: `npm run build`

Expected: PASS.

- [ ] **Step 6: Commit et**

```bash
git add src/layouts/DashboardLayout.tsx src/index.css src/pages/photoLibraryModel.test.mjs
git commit -m "feat(shell): add modern responsive PhotoOS layout"
```

### Task 3: Fotoğraflar üst alanı, istatistikler ve filtre şeridi

**Files:**
- Modify: `src/pages/PhotosPage.tsx`
- Create: `src/pages/PhotosPage.css`

**Interfaces:**
- Consumes: Task 1'in `SOURCE_OPTIONS`, `getPhotoDate`, `normalizeSourceType`, `matchesPhotoSearch` yardımcıları ve mevcut `/api/v1/photos`, `/count` çağrıları.
- Produces: `.photos-page`, `.photos-hero`, `.photo-stat-grid`, `.source-filter-strip`, `.time-filter-strip` DOM sözleşmesi.

- [ ] **Step 1: Sayfa yapısı için başarısız statik test yaz**

Test dosyasında `PhotosPage.tsx` metninin `PhotosPage.css` importunu, `photos-hero`, `photo-stat-grid`, `source-filter-strip`, `aria-pressed` ve `pc_backup` kaynak kataloğu kullanımını içerdiğini doğrula.

- [ ] **Step 2: Testin başarısız olduğunu doğrula**

Run: `node --test src/pages/photoLibraryModel.test.mjs`

Expected: FAIL; yeni sınıflar ve CSS importu yok.

- [ ] **Step 3: Türetilmiş verileri ekle**

`searchQuery` durumunu ekle. Fotoğraf/video sayısını yüklü kümeden, albüm sayısını mevcut `albums` durumundan, tarih aralığını geçerli filtre kümesinden türet. Toplam sayı yüklü kümeden farklıysa etikette `Yüklenen X / Toplam Y` göster.

- [ ] **Step 4: Üst alanı ve filtreleri yeniden oluştur**

Inline stilleri kaldırarak hedef görseldeki başlık, arama, dört istatistik kartı, kaynak şeridi, yıl/ay şeridi ve işlem düğmelerini semantik butonlarla oluştur. Aktif filtre düğmelerine `aria-pressed` ver. Sıfır kayıtlı kaynakları katalogdan göstermeye devam et.

- [ ] **Step 5: Responsive CSS'i yaz**

Masaüstünde dört istatistik sütunu; 1100px altında iki; 640px altında tek sütun kullan. Kaynak ve zaman şeritlerini yatay kaydırılabilir yap. Arama ve işlem alanları küçük ekranlarda tam genişliğe geçsin.

- [ ] **Step 6: Test ve build çalıştır**

Run: `node --test src/pages/photoLibraryModel.test.mjs`

Expected: PASS.

Run: `npm run build`

Expected: PASS.

- [ ] **Step 7: Commit et**

```bash
git add src/pages/PhotosPage.tsx src/pages/PhotosPage.css src/pages/photoLibraryModel.test.mjs
git commit -m "feat(photos): redesign library controls and timeline"
```

### Task 4: Modern galeri, seçim ve boş/hata durumları

**Files:**
- Modify: `src/pages/PhotosPage.tsx`
- Modify: `src/pages/PhotosPage.css`

**Interfaces:**
- Consumes: Task 3 filtrelenmiş sonuçları ve mevcut `selectedFiles`, `selected`, `visibleLimit` durumları.
- Produces: `.photo-gallery-grid`, `.photo-tile`, `.photo-source-badge`, `.photo-empty-state`, `.photo-error-state`.

- [ ] **Step 1: Galeri sözleşmesi için başarısız test ekle**

Testte galeri ve durum sınıflarını, küçük resim `onError` işleyicisini, seçim checkbox erişilebilir adını ve “Filtreleri temizle” metnini doğrula.

- [ ] **Step 2: Testin başarısız olduğunu doğrula**

Run: `node --test src/pages/photoLibraryModel.test.mjs`

Expected: FAIL; yeni galeri sözleşmesi yok.

- [ ] **Step 3: Galeri kartlarını dönüştür**

Mevcut thumbnail URL'sini, video işaretini, kaynak rozetini ve seçim davranışını koruyarak kartları CSS sınıflarına taşı. Kırık thumbnail durumunda kart içinde kaynak ikonlu placeholder göster; diğer kartları etkileme.

- [ ] **Step 4: Boş ve hata durumlarını ekle**

API hatasında yeniden deneme düğmesi göster. Filtre sonucu boşsa aktif kaynak etiketini içeren mesaj ve yalnızca istemci filtrelerini temizleyen düğme göster. `pc_backup` sıfır sonuçta başka kaynak verisi gösterme.

- [ ] **Step 5: Görüntüleyici regresyonunu önle**

Mevcut `goNext`, `goPrev`, Escape, silme, metadata, ZIP ve albüme ekleme fonksiyonlarını koru. Galeri yeniden düzenlemesi sırasında bu fonksiyonların adlarını veya çağrı parametrelerini değiştirme.

- [ ] **Step 6: Test, lint ve build çalıştır**

Run: `node --test src/pages/photoLibraryModel.test.mjs`

Expected: PASS.

Run: `npm run lint`

Expected: yeni dosyalarda yeni error yok; var olan ihlaller ayrı listelenir.

Run: `npm run build`

Expected: PASS ve `dist/index.html` oluşur.

- [ ] **Step 7: Commit et**

```bash
git add src/pages/PhotosPage.tsx src/pages/PhotosPage.css src/pages/photoLibraryModel.test.mjs
git commit -m "feat(photos): add responsive source-aware gallery"
```

### Task 5: Önizleme ve canlı API smoke doğrulaması

**Files:**
- Create: `reports/photo-library-smoke-20260920.md`

**Interfaces:**
- Consumes: Task 4 üretim `dist` dizini ve canlı PhotoOS API.
- Produces: HTTP, filtre, responsive ve regresyon sonuçlarını içeren doğrulama raporu.

- [ ] **Step 1: Üretim çıktısını izole önizleme dizinine kopyala**

```bash
mkdir -p /home/photoos/.photoos-photos-preview/frontend
rsync -a --delete client/dist/ /home/photoos/.photoos-photos-preview/frontend/
```

- [ ] **Step 2: Statik önizlemeyi canlı servisten ayrı portta başlat**

```bash
/home/photoos/.local/nodejs/node-v24.21.0-linux-x64/bin/npx vite preview --host 0.0.0.0 --port 5173
```

Expected: canlı 8080 servisi kesilmeden önizleme 5173 portunda açılır.

- [ ] **Step 3: API smoke kontrollerini çalıştır**

Geçerli token ile aşağıdaki isteklerin HTTP 200 ve beklenen kaynak davranışını verdiğini rapora kaydet:

```bash
curl -sS -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/
```

Tarayıcıdaki yetkili oturumda `/api/v1/photos/count` isteğinin HTTP 200 döndüğünü; `camera`, `whatsapp_received`, `screenshot`, `other` filtrelerini ve boş `pc_backup` filtresini kontrol et.

- [ ] **Step 4: Responsive kontrol yap**

1920×1080, 1366×768, 768×1024 ve 390×844 boyutlarında menü, filtre şeritleri, galeri ve görüntüleyicide yatay taşma olmadığını raporla.

- [ ] **Step 5: Regresyon kontrolü yap**

Seçim modu, ZIP indirme, albüme ekleme, önceki/sonraki, Escape ve silme onayını test et. Silme testi için üretim verisini silme; onay penceresini iptal ederek akışı doğrula.

- [ ] **Step 6: Raporu commit et**

```bash
git add reports/photo-library-smoke-20260920.md
git commit -m "test(photos): record redesign smoke verification"
```

### Task 6: Geri dönüşlü release kurulumu

**Files:**
- Create: `/opt/photoos/releases/1.2.2-photo-library/client/dist/`
- Preserve: `/opt/photoos/releases/1.2.1-wave2-334747e60a9a/`

**Interfaces:**
- Consumes: Doğrulanmış Task 5 dist çıktısı ve mevcut `1.2.1-wave2-334747e60a9a` release.
- Produces: `1.2.2-photo-library` release ve doğrulanmış `/opt/photoos/current` hedefi.

- [ ] **Step 1: Mevcut hedefi ve sağlık durumunu kaydet**

```bash
readlink -f /opt/photoos/current
systemctl is-active photoos.service
curl -sS -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/
```

Expected: hedef `1.2.1-wave2-334747e60a9a`, servis `active`, HTTP `200`.

- [ ] **Step 2: Yeni release'i eskiden kopyalayarak oluştur**

```bash
sudo cp -a /opt/photoos/releases/1.2.1-wave2-334747e60a9a /opt/photoos/releases/1.2.2-photo-library
sudo rsync -a --delete client/dist/ /opt/photoos/releases/1.2.2-photo-library/client/dist/
```

- [ ] **Step 3: Release sahiplik ve dosya kontrollerini yap**

```bash
sudo chown -R root:root /opt/photoos/releases/1.2.2-photo-library
test -f /opt/photoos/releases/1.2.2-photo-library/client/dist/index.html
```

Expected: `test` exit 0.

- [ ] **Step 4: Current bağlantısını atomik değiştir ve servisi yeniden başlat**

```bash
sudo ln -sfn /opt/photoos/releases/1.2.2-photo-library /opt/photoos/current.next
sudo mv -Tf /opt/photoos/current.next /opt/photoos/current
sudo systemctl restart photoos.service
```

- [ ] **Step 5: Sağlık kontrolünü çalıştır**

```bash
systemctl is-active photoos.service
curl -sS -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/
```

Expected: `active` ve `200`. Ardından tarayıcıdaki yetkili oturumla photos count isteğinin `200` döndüğünü doğrula.

- [ ] **Step 6: Başarısızlık halinde geri dön**

Yalnızca Step 5 başarısızsa:

```bash
sudo ln -sfn /opt/photoos/releases/1.2.1-wave2-334747e60a9a /opt/photoos/current.next
sudo mv -Tf /opt/photoos/current.next /opt/photoos/current
sudo systemctl restart photoos.service
```

Ardından Step 5 sağlık kontrollerini tekrar çalıştır ve sonucu rapora ekle.

- [ ] **Step 7: Son canlı görünüm kontrolünü tamamla**

8080 üzerindeki Fotoğraflar sayfasını masaüstü ve 390px mobil görünümde aç. Kaynak filtrelerinin gerçek sayılarla çalıştığını ve PC Backup'ın yanlış kayıt göstermediğini doğrula.
