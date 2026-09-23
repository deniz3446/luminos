import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
    SOURCE_OPTIONS,
    filterPhotoCollection,
    normalizeSourceType,
} from "./photoLibraryModel.ts";

const basePhoto = {
    filename: "IMG_1001.jpg",
    original_name: "Tatil.jpg",
    owner_username: "deniz",
    source_type: "camera",
    device_name: "Telefon",
    mime_type: "image/jpeg",
    taken_at: "2026-07-20 10:30:00",
    uploaded_at: "2026-07-21 10:30:00",
};

test("ayrıntılı kaynak kataloğunu tanımlar", () => {
    const values = SOURCE_OPTIONS.map((item) => item.value);
    for (const value of ["camera", "whatsapp_received", "whatsapp_sent", "screenshot", "download", "telegram", "pc_backup", "other"]) {
        assert.ok(values.includes(value));
    }
});

test("bilinmeyen ve boş kaynakları other olarak normalize eder", () => {
    assert.equal(normalizeSourceType("future_source"), "other");
    assert.equal(normalizeSourceType(undefined), "other");
});

test("arama ve filtrelerin kesişimini uygular", () => {
    const photos = [
        basePhoto,
        { ...basePhoto, filename: "WA.jpg", original_name: "Aile.jpg", source_type: "whatsapp_received", mime_type: "image/png" },
        { ...basePhoto, filename: "VID.mp4", original_name: "Tatil video.mp4", mime_type: "video/mp4" },
    ];

    const result = filterPhotoCollection(photos, {
        owner: "deniz",
        source: "camera",
        media: "photo",
        search: "tatil",
    });

    assert.deepEqual(result.map((photo) => photo.filename), ["IMG_1001.jpg"]);
});

test("modern fotoğraf merkezi erişilebilir arayüz sözleşmesini sunar", () => {
    const source = readFileSync(new URL("./PhotosLibraryPage.tsx", import.meta.url), "utf8");
    for (const contract of [
        "photos-hero",
        "photo-stat-grid",
        "source-filter-strip",
        "time-filter-strip",
        "photo-gallery-grid",
        "photo-empty-state",
        "aria-pressed",
        "Filtreleri temizle",
    ]) {
        assert.ok(source.includes(contract), `Eksik arayüz sözleşmesi: ${contract}`);
    }
});

test("mevcut fotoğraf yönetimi işlevlerini korur", () => {
    const source = readFileSync(new URL("./PhotosLibraryPage.tsx", import.meta.url), "utf8");
    for (const contract of ["addPhotosToAlbum", "deletePhoto", "goPrevious", "goNext", "downloadZip"]) {
        assert.ok(source.includes(contract), `Eksik yönetim işlevi: ${contract}`);
    }
});

test("sunucu sayfalaması, gruplanmış diğer filtresi ve işlem geri bildirimi vardır", () => {
    const source = readFileSync(new URL("./PhotosLibraryPage.tsx", import.meta.url), "utf8");
    for (const contract of [
        "currentOffset",
        "setOffset",
        'source !== "other"',
        "operationBusy",
        "operationMessage",
        "photos.length < total",
    ]) {
        assert.ok(source.includes(contract), `Eksik güvenli işlem sözleşmesi: ${contract}`);
    }
});
