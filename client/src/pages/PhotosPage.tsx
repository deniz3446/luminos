import { useEffect, useMemo, useState } from "react";
import { addPhotosToAlbum, getAlbums, type Album } from "../api/albums";

type Photo = {
    id: number;
    filename: string;
    original_name: string;
    url: string;
    size_bytes: number;
    mime_type: string;
    uploaded_at: string;
    taken_at?: string | null;
    user_id: number;
    owner_username: string;
    source_type: string;
    source_path?: string | null;
    device_name?: string | null;
};

import { API_URL } from "../api/client";
const PAGE_SIZE = 500;

const SOURCE_FILTER_OPTIONS = [
    { value: "all", label: "🗂️ Tümü" },
    { value: "camera", label: "📷 Kamera" },
    { value: "whatsapp", label: "💬 WhatsApp" },
    { value: "screenshot", label: "📱 Ekran Görüntüleri" },
    { value: "download", label: "📥 İndirilenler" },
    { value: "telegram", label: "✈️ Telegram" },
    { value: "other", label: "🖼️ Diğer" },
];

const SOURCE_LABEL_OPTIONS = [
    { value: "camera", label: "📷 Kamera" },
    { value: "whatsapp_received", label: "💬 WhatsApp Gelen" },
    { value: "whatsapp_sent", label: "📤 WhatsApp Gönderilen" },
    { value: "screenshot", label: "📱 Ekran Görüntüleri" },
    { value: "download", label: "📥 İndirilenler" },
    { value: "telegram", label: "✈️ Telegram" },
    { value: "other", label: "🖼️ Diğer" },
];

function sourceIcon(sourceType?: string) {
    switch (sourceType) {
        case "camera":
            return "📷";
        case "whatsapp_received":
            return "💬";
        case "whatsapp_sent":
            return "📤";
        case "screenshot":
            return "📱";
        case "download":
            return "📥";
        case "telegram":
            return "✈️";
        default:
            return "🖼️";
    }
}

function sourceLabel(sourceType?: string) {
    const option = SOURCE_LABEL_OPTIONS.find(
        (item) => item.value === sourceType
    );

    return option?.label.replace(
        /^[^ ]+ /,
        ""
    ) ?? "Diğer";
}

function isVideo(photo: Photo) {
    return photo.mime_type?.startsWith("video/");
}

function parsePhotoDate(raw?: string | null) {
    if (!raw) return null;

    const fixed = raw.replace(" ", "T");
    const date = new Date(fixed);

    return Number.isNaN(date.getTime()) ? null : date;
}

function getPhotoDate(photo: Photo) {
    return parsePhotoDate(photo.taken_at) || parsePhotoDate(photo.uploaded_at);
}

function monthName(month: number) {
    return new Date(2026, month, 1).toLocaleDateString("tr-TR", {
        month: "long",
    });
}

function formatPhotoDate(photo: Photo) {
    const date = getPhotoDate(photo);

    if (!date) return "Tarih bilinmiyor";

    return date.toLocaleString("tr-TR", {
        day: "2-digit",
        month: "long",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
    });
}

export default function PhotosPage() {
    const [photos, setPhotos] = useState<Photo[]>([]);
    const [photosTotal, setPhotosTotal] = useState(0);
    const [offset, setOffset] = useState(0);
    const [loading, setLoading] = useState(false);
    const [selected, setSelected] = useState<Photo | null>(null);
    const [ownerFilter, setOwnerFilter] = useState("all");
    const [sourceFilter, setSourceFilter] = useState("all");
    const [mediaFilter, setMediaFilter] = useState("all");
    const [selectedYear, setSelectedYear] = useState("");
    const [selectedMonth, setSelectedMonth] = useState<number | null>(null);
    const [visibleLimit, setVisibleLimit] = useState(80);
    const [error, setError] = useState("");
    const [selectionMode, setSelectionMode] = useState(false);
    const [selectedFiles, setSelectedFiles] = useState<string[]>([]);
    const [downloadingZip, setDownloadingZip] = useState(false);
    const [albums, setAlbums] = useState<Album[]>([]);
    const [selectedAlbumId, setSelectedAlbumId] = useState("");
    const [addingToAlbum, setAddingToAlbum] = useState(false);

    async function loadPhotos(reset = false) {
        try {
            setLoading(true);
            setError("");

            const currentOffset = reset ? 0 : offset;
            const token = localStorage.getItem("token");

            const query = new URLSearchParams({
                limit: String(PAGE_SIZE),
                offset: String(currentOffset),
            });

            const countQuery = new URLSearchParams();

            if (sourceFilter !== "all") {
                query.set("source_type", sourceFilter);
                countQuery.set("source_type", sourceFilter);
            }

            if (mediaFilter !== "all") {
                query.set("media_type", mediaFilter);
                countQuery.set("media_type", mediaFilter);
            }

            const [photosRes, countRes] = await Promise.all([
                fetch(
                    `${API_URL}/api/v1/photos?${query.toString()}`,
                    {
                        headers: token
                            ? {
                                Authorization:
                                    `Bearer ${token}`,
                            }
                            : {},
                    }
                ),
                fetch(
                    `${API_URL}/api/v1/photos/count?${countQuery.toString()}`,
                    {
                        headers: token
                            ? {
                                Authorization:
                                    `Bearer ${token}`,
                            }
                            : {},
                    }
                ),
            ]);

            if (!photosRes.ok) {
                throw new Error(`Fotoğraflar alınamadı: ${photosRes.status}`);
            }

            if (!countRes.ok) {
                throw new Error(`Fotoğraf sayısı alınamadı: ${countRes.status}`);
            }

            const photosData = await photosRes.json();
            const countData = await countRes.json();

            const incoming = Array.isArray(photosData) ? photosData : [];

            setPhotos((prev) => (reset ? incoming : [...prev, ...incoming]));
            setPhotosTotal(Number(countData.count ?? 0));
            setOffset(currentOffset + incoming.length);

            if (reset) {
                setSelectedFiles([]);
            }
        } catch (err) {
            console.error(err);
            setError("Fotoğraflar yüklenemedi.");
        } finally {
            setLoading(false);
        }
    }


    function toggleSelectionMode() {
        setSelectionMode((prev) => {
            if (prev) {
                setSelectedFiles([]);
            }
            return !prev;
        });
    }

    function togglePhotoSelection(filename: string) {
        setSelectedFiles((prev) =>
            prev.includes(filename)
                ? prev.filter((f) => f !== filename)
                : [...prev, filename]
        );
    }

    async function downloadSelectedZip() {
        if (selectedFiles.length === 0) {
            alert("Önce indirilecek fotoğraf/video seç.");
            return;
        }

        try {
            setDownloadingZip(true);

            const token = localStorage.getItem("token");

            const res = await fetch(`${API_URL}/api/v1/photos/download-zip`, {
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                    ...(token ? { Authorization: `Bearer ${token}` } : {}),
                },
                body: JSON.stringify({
                    filenames: selectedFiles,
                }),
            });

            if (!res.ok) {
                throw new Error("ZIP indirilemedi");
            }

            const blob = await res.blob();
            const url = window.URL.createObjectURL(blob);

            const a = document.createElement("a");
            a.href = url;
            a.download = "photoos-download.zip";
            document.body.appendChild(a);
            a.click();
            a.remove();

            window.URL.revokeObjectURL(url);
        } catch (err) {
            console.error(err);
            alert("ZIP indirme başarısız oldu.");
        } finally {
            setDownloadingZip(false);
        }
    }


    async function addSelectedToAlbum() {
        if (!selectedAlbumId) {
            alert("Önce albüm seç.");
            return;
        }

        const selectedPhotoIds = photos
            .filter((p) => selectedFiles.includes(p.filename))
            .map((p) => p.id);

        if (selectedPhotoIds.length === 0) {
            alert("Önce fotoğraf/video seç.");
            return;
        }

        try {
            setAddingToAlbum(true);

            await addPhotosToAlbum(Number(selectedAlbumId), selectedPhotoIds);

            alert(`${selectedPhotoIds.length} medya albüme eklendi.`);
            setSelectedFiles([]);
            setSelectedAlbumId("");
        } catch (err) {
            console.error(err);
            alert("Albüme ekleme başarısız oldu.");
        } finally {
            setAddingToAlbum(false);
        }
    }

    function selectWholeYear() {
        const allYear = selectedYearPhotos.map((p) => p.filename);

        setSelectedFiles((prev) => {
            const set = new Set(prev);
            for (const filename of allYear) {
                set.add(filename);
            }
            return Array.from(set);
        });
    }

    function clearWholeYear() {
        const yearSet = new Set(selectedYearPhotos.map((p) => p.filename));
        setSelectedFiles((prev) => prev.filter((f) => !yearSet.has(f)));
    }

    function selectWholeMonth() {
        const allMonth = selectedMonthPhotos.map((p) => p.filename);

        setSelectedFiles((prev) => {
            const set = new Set(prev);
            for (const filename of allMonth) {
                set.add(filename);
            }
            return Array.from(set);
        });
    }

    function clearWholeMonth() {
        const monthSet = new Set(selectedMonthPhotos.map((p) => p.filename));
        setSelectedFiles((prev) => prev.filter((f) => !monthSet.has(f)));
    }

    function clearAllSelections() {
        setSelectedFiles([]);
    }

    async function deletePhoto(photo: Photo) {
        if (!confirm("Bu dosya silinsin mi?")) return;

        try {
            const token = localStorage.getItem("token");

            const res = await fetch(`${API_URL}/api/v1/photos/file/${photo.filename}`, {
                method: "DELETE",
                headers: token ? { Authorization: `Bearer ${token}` } : {},
            });

            if (!res.ok) {
                throw new Error("Silme başarısız");
            }

            setSelected(null);
            await loadPhotos(true);
        } catch (err) {
            console.error(err);
            alert("Silme başarısız oldu.");
        }
    }

    useEffect(() => {
        setPhotos([]);
        setOffset(0);
        setSelectedYear("");
        setSelectedMonth(null);
        setVisibleLimit(80);
        loadPhotos(true);
    }, [sourceFilter, mediaFilter]);

    useEffect(() => {
        async function loadAlbums() {
            try {
                const data = await getAlbums();
                setAlbums(Array.isArray(data) ? data : []);
            } catch (err) {
                console.error("Albüm listesi alınamadı", err);
            }
        }

        loadAlbums();
    }, []);

    useEffect(() => {
        setVisibleLimit(80);
    }, [
        ownerFilter,
        sourceFilter,
        mediaFilter,
        selectedYear,
        selectedMonth,
    ]);

    const owners = useMemo(() => {
        return Array.from(
            new Set(photos.map((p) => p.owner_username || "Bilinmeyen"))
        ).sort((a, b) => a.localeCompare(b, "tr"));
    }, [photos]);

    const filteredPhotos = useMemo(() => {
        if (ownerFilter === "all") return photos;

        return photos.filter(
            (p) => (p.owner_username || "Bilinmeyen") === ownerFilter
        );
    }, [photos, ownerFilter]);

    const years = useMemo(() => {
        const map = new Map<string, Photo[]>();

        for (const photo of filteredPhotos) {
            const date = getPhotoDate(photo);
            const year = date ? String(date.getFullYear()) : "Bilinmeyen";

            map.set(year, [...(map.get(year) ?? []), photo]);
        }

        return Array.from(map.entries())
            .map(([year, items]) => ({
                year,
                items: items.sort(
                    (a, b) =>
                        (getPhotoDate(b)?.getTime() ?? 0) -
                        (getPhotoDate(a)?.getTime() ?? 0)
                ),
            }))
            .sort((a, b) => {
                if (a.year === "Bilinmeyen") return 1;
                if (b.year === "Bilinmeyen") return -1;
                return Number(b.year) - Number(a.year);
            });
    }, [filteredPhotos]);

    useEffect(() => {
        if (!selectedYear && years.length > 0) {
            setSelectedYear(years[0].year);
        }

        if (selectedYear && !years.some((y) => y.year === selectedYear)) {
            setSelectedYear(years[0]?.year ?? "");
        }
    }, [years, selectedYear]);

    const selectedYearPhotos = useMemo(() => {
        return years.find((y) => y.year === selectedYear)?.items ?? [];
    }, [years, selectedYear]);

    const months = useMemo(() => {
        const map = new Map<number, Photo[]>();

        for (const photo of selectedYearPhotos) {
            const date = getPhotoDate(photo);
            if (!date) continue;

            const month = date.getMonth();
            map.set(month, [...(map.get(month) ?? []), photo]);
        }

        return Array.from(map.entries())
            .map(([month, items]) => ({
                month,
                name: monthName(month),
                items: items.sort(
                    (a, b) =>
                        (getPhotoDate(b)?.getTime() ?? 0) -
                        (getPhotoDate(a)?.getTime() ?? 0)
                ),
            }))
            .sort((a, b) => b.month - a.month);
    }, [selectedYearPhotos]);

    useEffect(() => {
        if (months.length === 0) {
            setSelectedMonth(null);
            return;
        }

        setSelectedMonth((old) => {
            if (old !== null && months.some((m) => m.month === old)) {
                return old;
            }

            return months[0].month;
        });
    }, [months]);

    const selectedMonthPhotos = useMemo(() => {
        return months.find((m) => m.month === selectedMonth)?.items ?? [];
    }, [months, selectedMonth]);

    const visiblePhotos = useMemo(() => {
        return selectedMonthPhotos.slice(0, visibleLimit);
    }, [selectedMonthPhotos, visibleLimit]);

    function selectedIndex() {
        if (!selected) return -1;

        return visiblePhotos.findIndex((p) => p.filename === selected.filename);
    }

    function goNext() {
        if (!selected || visiblePhotos.length === 0) return;

        const index = selectedIndex();
        if (index === -1) return;

        setSelected(visiblePhotos[(index + 1) % visiblePhotos.length]);
    }

    function goPrev() {
        if (!selected || visiblePhotos.length === 0) return;

        const index = selectedIndex();
        if (index === -1) return;

        setSelected(visiblePhotos[index === 0 ? visiblePhotos.length - 1 : index - 1]);
    }

    useEffect(() => {
        function onKeyDown(e: KeyboardEvent) {
            if (!selected) return;

            if (e.key === "Escape") {
                e.preventDefault();
                setSelected(null);
            }

            if (e.key === "ArrowRight") {
                e.preventDefault();
                goNext();
            }

            if (e.key === "ArrowLeft") {
                e.preventDefault();
                goPrev();
            }
        }

        window.addEventListener("keydown", onKeyDown);

        return () => window.removeEventListener("keydown", onKeyDown);
    }, [selected, visiblePhotos]);

    return (
        <div>
            <div
                style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    marginBottom: 24,
                    gap: 14,
                    flexWrap: "wrap",
                }}
            >
                <div>
                    <h1 style={{ margin: 0, fontSize: 32 }}>Fotoğraflar</h1>
                    <p style={{ margin: "6px 0 0", color: "#fecaca" }}>
                        Yüklenen: {photos.length} / {photosTotal}
                    </p>
                    {error && (
                        <p style={{ margin: "6px 0 0", color: "#ef4444" }}>
                            {error}
                        </p>
                    )}
                </div>

                <div
                    style={{
                        display: "flex",
                        gap: 12,
                        alignItems: "center",
                        flexWrap: "wrap",
                    }}
                >
                    <button
                        onClick={() => loadPhotos(true)}
                        disabled={loading}
                        style={{
                            height: 44,
                            borderRadius: 14,
                            border: "1px solid #7f1d1d",
                            background: loading ? "#3f3f46" : "#991b1b",
                            color: "#fff",
                            padding: "0 16px",
                            fontWeight: 800,
                            cursor: loading ? "not-allowed" : "pointer",
                        }}
                    >
                        {loading ? "Yükleniyor..." : "Yenile"}
                    </button>

                    <button
                        onClick={toggleSelectionMode}
                        style={{
                            height: 44,
                            borderRadius: 14,
                            border: "1px solid #ef4444",
                            background: selectionMode ? "#991b1b" : "#180507",
                            color: "#fff",
                            padding: "0 16px",
                            fontWeight: 800,
                            cursor: "pointer",
                        }}
                    >
                        {selectionMode ? "Seçim Modunu Kapat" : "Seçim Modu"}
                    </button>

                    {selectionMode && (
                        <>
                            <div
                                style={{
                                    padding: "10px 14px",
                                    borderRadius: 14,
                                    background: "#180507",
                                    border: "1px solid rgba(239,68,68,0.25)",
                                    color: "#fecaca",
                                    fontWeight: 700,
                                }}
                            >
                                Seçilen: {selectedFiles.length}
                            </div>

                            <button
                                onClick={downloadSelectedZip}
                                disabled={selectedFiles.length === 0 || downloadingZip}
                                style={{
                                    height: 44,
                                    borderRadius: 14,
                                    border: "1px solid #ef4444",
                                    background:
                                        selectedFiles.length === 0 || downloadingZip
                                            ? "#3b0a0d"
                                            : "#991b1b",
                                    color: "#fff",
                                    padding: "0 16px",
                                    fontWeight: 800,
                                    cursor:
                                        selectedFiles.length === 0 || downloadingZip
                                            ? "not-allowed"
                                            : "pointer",
                                }}
                            >
                                {downloadingZip ? "ZIP hazırlanıyor..." : "Seçilenleri ZIP indir"}
                            </button>

                            <select
                                value={selectedAlbumId}
                                onChange={(e) => setSelectedAlbumId(e.target.value)}
                                style={{
                                    height: 44,
                                    borderRadius: 14,
                                    background: "#120406",
                                    color: "#fff",
                                    border: "1px solid #7f1d1d",
                                    padding: "0 14px",
                                    minWidth: 220,
                                }}
                            >
                                <option value="">Albüme ekle...</option>
                                {albums.map((album) => (
                                    <option key={album.id} value={album.id}>
                                        {album.title}
                                    </option>
                                ))}
                            </select>

                            <button
                                onClick={addSelectedToAlbum}
                                disabled={
                                    selectedFiles.length === 0 ||
                                    !selectedAlbumId ||
                                    addingToAlbum
                                }
                                style={{
                                    height: 44,
                                    borderRadius: 14,
                                    border: "1px solid #7f1d1d",
                                    background:
                                        selectedFiles.length === 0 ||
                                        !selectedAlbumId ||
                                        addingToAlbum
                                            ? "#3b0a0d"
                                            : "#7f1d1d",
                                    color: "#fff",
                                    padding: "0 16px",
                                    fontWeight: 800,
                                    cursor:
                                        selectedFiles.length === 0 ||
                                        !selectedAlbumId ||
                                        addingToAlbum
                                            ? "not-allowed"
                                            : "pointer",
                                }}
                            >
                                {addingToAlbum ? "Ekleniyor..." : "Albüme Ekle"}
                            </button>

                            <button
                                onClick={clearAllSelections}
                                disabled={selectedFiles.length === 0}
                                style={{
                                    height: 44,
                                    borderRadius: 14,
                                    border: "1px solid #3f3f46",
                                    background: selectedFiles.length === 0 ? "#111" : "#18181b",
                                    color: "#fff",
                                    padding: "0 16px",
                                    fontWeight: 800,
                                    cursor: selectedFiles.length === 0 ? "not-allowed" : "pointer",
                                }}
                            >
                                Tüm seçimi temizle
                            </button>
                        </>
                    )}

                    <div
                        role="group"
                        aria-label="Fotoğraf kaynakları"
                        style={{
                            display: "flex",
                            flexWrap: "wrap",
                            gap: 8,
                            width: "100%",
                        }}
                    >
                        {SOURCE_FILTER_OPTIONS.map((option) => {
                            const active = sourceFilter === option.value;
                            return (
                                <button
                                    key={option.value}
                                    type="button"
                                    data-source-filter={option.value}
                                    aria-pressed={active}
                                    onClick={() => setSourceFilter(option.value)}
                                    style={{
                                        minHeight: 44,
                                        borderRadius: 14,
                                        border: active ? "1px solid #ef4444" : "1px solid #3f3f46",
                                        background: active ? "#7f1d1d" : "#18181b",
                                        color: "#fff",
                                        padding: "0 15px",
                                        fontWeight: 800,
                                        cursor: "pointer",
                                    }}
                                >
                                    {option.label}
                                </button>
                            );
                        })}
                    </div>

                    <select
                        value={mediaFilter}
                        onChange={(e) =>
                            setMediaFilter(e.target.value)
                        }
                        style={{
                            height: 44,
                            borderRadius: 14,
                            background: "#120406",
                            color: "#fff",
                            border: "1px solid #7f1d1d",
                            padding: "0 16px",
                            minWidth: 180,
                        }}
                    >
                        <option value="all">
                            Tüm medya
                        </option>
                        <option value="photo">
                            📷 Fotoğraflar
                        </option>
                        <option value="video">
                            🎥 Videolar
                        </option>
                    </select>

                    <select
                        value={ownerFilter}
                        onChange={(e) => setOwnerFilter(e.target.value)}
                        style={{
                            height: 44,
                            borderRadius: 14,
                            background: "#120406",
                            color: "#fff",
                            border: "1px solid #7f1d1d",
                            padding: "0 16px",
                            minWidth: 220,
                        }}
                    >
                        <option value="all">Tüm kullanıcılar</option>
                        {owners.map((owner) => (
                            <option key={owner} value={owner}>
                                {owner}
                            </option>
                        ))}
                    </select>
                </div>
            </div>

            <div
                style={{
                    display: "grid",
                    gridTemplateColumns: "repeat(5, minmax(0, 1fr))",
                    gap: 14,
                    marginBottom: 22,
                }}
            >
                {years.map(({ year, items }) => {
                    const cover = items[0];
                    const active = year === selectedYear;

                    return (
                        <button
                            key={year}
                            onClick={() => {
                                setSelectedYear(year);
                                setSelectedMonth(null);
                            }}
                            style={{
                                textAlign: "left",
                                borderRadius: 18,
                                overflow: "hidden",
                                background: active
                                    ? "linear-gradient(180deg, #22080b 0%, #120406 100%)"
                                    : "linear-gradient(180deg, #141414 0%, #0b0b0b 100%)",
                                border: active
                                    ? "1px solid rgba(239,68,68,0.95)"
                                    : "1px solid rgba(255,255,255,0.08)",
                                color: "#fff",
                                padding: 0,
                                cursor: "pointer",
                                boxShadow: active
                                    ? "0 0 0 1px rgba(239,68,68,0.18), 0 16px 40px rgba(127,29,29,0.35)"
                                    : "0 10px 24px rgba(0,0,0,0.22)",
                            }}
                        >
                            <div style={{ padding: "14px 16px" }}>
                                <div style={{ fontSize: 20, fontWeight: 900 }}>
                                    📅 {year}
                                </div>
                                <div style={{ color: "#fca5a5", marginTop: 4 }}>
                                    {items.length} medya
                                </div>
                            </div>

                            {cover && !isVideo(cover) && (
                                <img
                                    src={`${API_URL}/api/v1/photos/thumb/${cover.filename}`}
                                    alt={cover.filename}
                                    loading="lazy"
                                    decoding="async"
                                    style={{
                                        width: "100%",
                                        height: 110,
                                        objectFit: "cover",
                                        display: "block",
                                        opacity: active ? 0.96 : 0.78,
                                    }}
                                />
                            )}
                        </button>
                    );
                })}
            </div>


            {selectionMode && (
                <div
                    style={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                        gap: 12,
                        flexWrap: "wrap",
                        marginBottom: 16,
                        padding: 14,
                        borderRadius: 18,
                        background: "linear-gradient(145deg, #120406, #080808)",
                        border: "1px solid rgba(239,68,68,0.22)",
                    }}
                >
                    <div style={{ color: "#fecaca", fontWeight: 800 }}>
                        Seçili yıl: {selectedYear || "-"} — {selectedYearPhotos.length} medya
                    </div>

                    <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
                        <button
                            onClick={selectWholeYear}
                            style={{
                                borderRadius: 12,
                                border: "1px solid #7f1d1d",
                                background: "#1a0a0d",
                                color: "#fff",
                                padding: "10px 14px",
                                fontWeight: 700,
                                cursor: "pointer",
                            }}
                        >
                            Yılın tümünü seç
                        </button>

                        <button
                            onClick={clearWholeYear}
                            style={{
                                borderRadius: 12,
                                border: "1px solid #3f3f46",
                                background: "#111",
                                color: "#fff",
                                padding: "10px 14px",
                                fontWeight: 700,
                                cursor: "pointer",
                            }}
                        >
                            Yıl seçimini temizle
                        </button>
                    </div>
                </div>
            )}

            <div
                style={{
                    border: "1px solid rgba(239,68,68,0.2)",
                    borderRadius: 20,
                    background: "linear-gradient(145deg, #100406, #050505)",
                    padding: 16,
                }}
            >
                <div
                    style={{
                        display: "grid",
                        gridTemplateColumns: "repeat(5, minmax(0, 1fr))",
                        gap: 12,
                        marginBottom: 16,
                    }}
                >
                    {months.map((month) => (
                        <button
                            key={month.month}
                            onClick={() => setSelectedMonth(month.month)}
                            style={{
                                minWidth: 150,
                                borderRadius: 14,
                                padding: "12px 14px",
                                background:
                                    selectedMonth === month.month
                                        ? "linear-gradient(180deg, #b91c1c 0%, #7f1d1d 100%)"
                                        : "linear-gradient(180deg, #171717 0%, #101010 100%)",
                                color: "#fff",
                                border:
                                    selectedMonth === month.month
                                        ? "1px solid rgba(239,68,68,0.95)"
                                        : "1px solid rgba(255,255,255,0.08)",
                                cursor: "pointer",
                                textAlign: "left",
                            }}
                        >
                            <b>{month.name}</b>
                            <div style={{ color: "#fecaca", marginTop: 4, fontSize: 13 }}>
                                {month.items.length} medya
                            </div>
                        </button>
                    ))}
                </div>

                {selectionMode && selectedMonthPhotos.length > 0 && (
                    <div
                        style={{
                            display: "flex",
                            justifyContent: "space-between",
                            alignItems: "center",
                            gap: 12,
                            flexWrap: "wrap",
                            marginBottom: 16,
                            padding: 14,
                            borderRadius: 18,
                            background: "linear-gradient(145deg, #120406, #080808)",
                            border: "1px solid rgba(239,68,68,0.22)",
                        }}
                    >
                        <div style={{ color: "#fecaca", fontWeight: 800 }}>
                            Seçili ay: {months.find((m) => m.month === selectedMonth)?.name || "-"} — {selectedMonthPhotos.length} medya
                        </div>

                        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
                            <button
                                onClick={selectWholeMonth}
                                style={{
                                    borderRadius: 12,
                                    border: "1px solid #7f1d1d",
                                    background: "#1a0a0d",
                                    color: "#fff",
                                    padding: "10px 14px",
                                    fontWeight: 700,
                                    cursor: "pointer",
                                }}
                            >
                                Ayın tümünü seç
                            </button>

                            <button
                                onClick={clearWholeMonth}
                                style={{
                                    borderRadius: 12,
                                    border: "1px solid #3f3f46",
                                    background: "#111",
                                    color: "#fff",
                                    padding: "10px 14px",
                                    fontWeight: 700,
                                    cursor: "pointer",
                                }}
                            >
                                Ay seçimini temizle
                            </button>
                        </div>
                    </div>
                )}

                {selectedMonthPhotos.length === 0 ? (
                    <div
                        style={{
                            textAlign: "center",
                            padding: 36,
                            color: "#aaa",
                        }}
                    >
                        Bu seçimde medya yok.
                    </div>
                ) : (
                    <div
                        style={{
                            display: "grid",
                            gridTemplateColumns: "repeat(5, minmax(0, 1fr))",
                            gap: 14,
                        }}
                    >
                        {visiblePhotos.map((photo) => {
                            const video = isVideo(photo);
                            const checked = selectedFiles.includes(photo.filename);

                            return (
                                <div
                                    key={photo.filename}
                                    style={{
                                        height: 165,
                                        borderRadius: 16,
                                        overflow: "hidden",
                                        background: "#080808",
                                        border: checked
                                            ? "2px solid #ef4444"
                                            : "1px solid rgba(239,68,68,0.22)",
                                        position: "relative",
                                        boxShadow: checked
                                            ? "0 0 0 2px rgba(239,68,68,0.15)"
                                            : "none",
                                    }}
                                >
                                    <div
                                        title={sourceLabel(
                                            photo.source_type
                                        )}
                                        style={{
                                            position: "absolute",
                                            top: 8,
                                            right: 8,
                                            zIndex: 6,
                                            minWidth: 30,
                                            height: 30,
                                            padding: "0 8px",
                                            borderRadius: 999,
                                            display: "flex",
                                            alignItems: "center",
                                            justifyContent: "center",
                                            background:
                                                "rgba(0,0,0,0.72)",
                                            border:
                                                "1px solid rgba(255,255,255,0.22)",
                                            fontSize: 16,
                                        }}
                                    >
                                        {video
                                            ? "🎥"
                                            : sourceIcon(
                                                  photo.source_type
                                              )}
                                    </div>

                                    {selectionMode && (
                                        <button
                                            onClick={(e) => {
                                                e.stopPropagation();
                                                togglePhotoSelection(photo.filename);
                                            }}
                                            style={{
                                                position: "absolute",
                                                top: 8,
                                                left: 8,
                                                zIndex: 5,
                                                width: 28,
                                                height: 28,
                                                borderRadius: 999,
                                                border: checked
                                                    ? "2px solid #ef4444"
                                                    : "2px solid rgba(255,255,255,0.7)",
                                                background: checked ? "#ef4444" : "rgba(0,0,0,0.45)",
                                                color: "#fff",
                                                fontWeight: 900,
                                                cursor: "pointer",
                                            }}
                                        >
                                            {checked ? "✓" : ""}
                                        </button>
                                    )}

                                    <div
                                        onClick={() => {
                                            if (selectionMode) {
                                                togglePhotoSelection(photo.filename);
                                            } else {
                                                setSelected(photo);
                                            }
                                        }}
                                        style={{
                                            width: "100%",
                                            height: "100%",
                                            cursor: "pointer",
                                            position: "relative",
                                        }}
                                    >
                                        {video ? (
                                            <div
                                                style={{
                                                    width: "100%",
                                                    height: "100%",
                                                    display: "flex",
                                                    alignItems: "center",
                                                    justifyContent: "center",
                                                    fontSize: 42,
                                                    color: "#fff",
                                                    background: "#111",
                                                }}
                                            >
                                                ▶
                                            </div>
                                        ) : (
                                            <img
                                                src={`${API_URL}/api/v1/photos/thumb/${photo.filename}`}
                                                alt={photo.filename}
                                                loading="lazy"
                                                decoding="async"
                                                style={{
                                                    width: "100%",
                                                    height: "100%",
                                                    objectFit: "cover",
                                                    display: "block",
                                                }}
                                            />
                                        )}

                                        <div
                                            style={{
                                                position: "absolute",
                                                left: 0,
                                                right: 0,
                                                bottom: 0,
                                                padding: "8px 10px",
                                                background:
                                                    "linear-gradient(to top, rgba(0,0,0,0.75), transparent)",
                                                color: "#fecaca",
                                                fontSize: 12,
                                                fontWeight: 700,
                                            }}
                                        >
                                            {photo.owner_username || "Bilinmeyen"}
                                        </div>
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                )}

                {selectedMonthPhotos.length > visiblePhotos.length && (
                    <div
                        style={{
                            display: "flex",
                            justifyContent: "center",
                            marginTop: 18,
                        }}
                    >
                        <button
                            onClick={() => setVisibleLimit((prev) => prev + 80)}
                            style={{
                                borderRadius: 14,
                                border: "1px solid #7f1d1d",
                                background: "#1a0a0d",
                                color: "#fff",
                                padding: "12px 18px",
                                fontWeight: 800,
                                cursor: "pointer",
                            }}
                        >
                            Daha Fazla Göster ({visiblePhotos.length}/{selectedMonthPhotos.length})
                        </button>
                    </div>
                )}

                {photos.length < photosTotal && (
                    <div
                        style={{
                            display: "flex",
                            justifyContent: "center",
                            marginTop: 14,
                        }}
                    >
                        <button
                            onClick={() => loadPhotos(false)}
                            disabled={loading}
                            style={{
                                borderRadius: 14,
                                border: "1px solid #3f3f46",
                                background: loading ? "#111" : "#18181b",
                                color: "#fff",
                                padding: "12px 18px",
                                fontWeight: 800,
                                cursor: loading ? "not-allowed" : "pointer",
                            }}
                        >
                            {loading
                                ? "Yükleniyor..."
                                : `Sunucudan Daha Fazla Yükle (${photos.length}/${photosTotal})`}
                        </button>
                    </div>
                )}
            </div>

            {selected && (
                <div
                    onClick={() => setSelected(null)}
                    style={{
                        position: "fixed",
                        inset: 0,
                        zIndex: 9999,
                        background: "rgba(0,0,0,0.94)",
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        padding: 22,
                    }}
                >
                    <button
                        onClick={(e) => {
                            e.stopPropagation();
                            goPrev();
                        }}
                        style={{
                            position: "fixed",
                            left: 22,
                            top: "50%",
                            transform: "translateY(-50%)",
                            width: 52,
                            height: 52,
                            borderRadius: 999,
                            border: "1px solid rgba(239,68,68,0.55)",
                            background: "rgba(24,5,7,0.88)",
                            color: "#fff",
                            fontSize: 28,
                            cursor: "pointer",
                            zIndex: 10000,
                        }}
                    >
                        ‹
                    </button>

                    <button
                        onClick={(e) => {
                            e.stopPropagation();
                            goNext();
                        }}
                        style={{
                            position: "fixed",
                            right: 22,
                            top: "50%",
                            transform: "translateY(-50%)",
                            width: 52,
                            height: 52,
                            borderRadius: 999,
                            border: "1px solid rgba(239,68,68,0.55)",
                            background: "rgba(24,5,7,0.88)",
                            color: "#fff",
                            fontSize: 28,
                            cursor: "pointer",
                            zIndex: 10000,
                        }}
                    >
                        ›
                    </button>

                    <div
                        onClick={(e) => e.stopPropagation()}
                        style={{
                            width: "min(1180px, 94vw)",
                            maxHeight: "92vh",
                            display: "grid",
                            gridTemplateRows: "auto minmax(0, 1fr) auto",
                            background: "#070707",
                            border: "1px solid rgba(239,68,68,0.32)",
                            borderRadius: 24,
                            overflow: "hidden",
                        }}
                    >
                        <div
                            style={{
                                padding: "14px 18px",
                                display: "flex",
                                justifyContent: "space-between",
                                alignItems: "center",
                                gap: 14,
                                borderBottom: "1px solid rgba(239,68,68,0.18)",
                                background: "linear-gradient(90deg, #130406, #070707)",
                            }}
                        >
                            <div style={{ minWidth: 0 }}>
                                <div
                                    style={{
                                        color: "#fff",
                                        fontWeight: 900,
                                        whiteSpace: "nowrap",
                                        overflow: "hidden",
                                        textOverflow: "ellipsis",
                                    }}
                                >
                                    {selected.original_name || selected.filename}
                                </div>
                                <div style={{ color: "#fecaca", fontSize: 13, marginTop: 4 }}>
                                    {selected.owner_username || "Bilinmeyen"}
                                    {" · "}
                                    {formatPhotoDate(selected)}
                                    {" · "}
                                    {isVideo(selected)
                                        ? "Video"
                                        : sourceLabel(
                                              selected.source_type
                                          )}
                                </div>
                            </div>

                            <button
                                onClick={() => setSelected(null)}
                                style={{
                                    width: 38,
                                    height: 38,
                                    borderRadius: 999,
                                    border: "1px solid rgba(255,255,255,0.12)",
                                    background: "#151515",
                                    color: "#fff",
                                    cursor: "pointer",
                                    fontSize: 18,
                                }}
                            >
                                ✕
                            </button>
                        </div>

                        <div
                            style={{
                                minHeight: 0,
                                background: "#000",
                                display: "flex",
                                alignItems: "center",
                                justifyContent: "center",
                                padding: 16,
                            }}
                        >
                            {isVideo(selected) ? (
                                <video
                                    src={`${API_URL}${selected.url}`}
                                    controls
                                    autoPlay
                                    style={{
                                        maxWidth: "100%",
                                        maxHeight: "72vh",
                                        background: "#000",
                                    }}
                                />
                            ) : (
                                <img
                                    src={`${API_URL}${selected.url}`}
                                    alt={selected.filename}
                                    style={{
                                        maxWidth: "100%",
                                        maxHeight: "72vh",
                                        objectFit: "contain",
                                    }}
                                />
                            )}
                        </div>

                        <div
                            style={{
                                padding: 14,
                                display: "flex",
                                justifyContent: "space-between",
                                alignItems: "center",
                                gap: 12,
                                flexWrap: "wrap",
                                borderTop: "1px solid rgba(239,68,68,0.18)",
                                background: "#090909",
                            }}
                        >
                            <div style={{ color: "#aaa", fontSize: 13 }}>
                                ESC: kapat · ← / →: gezin
                            </div>

                            <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
                                <button
                                    onClick={goPrev}
                                    style={{
                                        borderRadius: 12,
                                        border: "1px solid #3f3f46",
                                        background: "#111",
                                        color: "#fff",
                                        padding: "10px 14px",
                                        fontWeight: 700,
                                        cursor: "pointer",
                                    }}
                                >
                                    ← Önceki
                                </button>

                                <button
                                    onClick={goNext}
                                    style={{
                                        borderRadius: 12,
                                        border: "1px solid #7f1d1d",
                                        background: "#1a0a0d",
                                        color: "#fff",
                                        padding: "10px 14px",
                                        fontWeight: 700,
                                        cursor: "pointer",
                                    }}
                                >
                                    Sonraki →
                                </button>

                                <button
                                    onClick={() => window.open(`${API_URL}${selected.url}`, "_blank")}
                                    style={{
                                        borderRadius: 12,
                                        border: "1px solid #7f1d1d",
                                        background: "#991b1b",
                                        color: "#fff",
                                        padding: "10px 14px",
                                        fontWeight: 700,
                                        cursor: "pointer",
                                    }}
                                >
                                    Yeni Sekmede Aç
                                </button>

                                <button
                                    onClick={() => deletePhoto(selected)}
                                    style={{
                                        borderRadius: 12,
                                        border: "1px solid #991b1b",
                                        background: "#450a0a",
                                        color: "#fff",
                                        padding: "10px 14px",
                                        fontWeight: 700,
                                        cursor: "pointer",
                                    }}
                                >
                                    Sil
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
