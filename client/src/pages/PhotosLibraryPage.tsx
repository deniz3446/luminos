import { useEffect, useMemo, useState } from "react";
import { addPhotosToAlbum, getAlbums, type Album } from "../api/albums";
import { API_URL } from "../api/client";
import {
    SOURCE_OPTIONS,
    filterPhotoCollection,
    getPhotoDate,
    normalizeSourceType,
} from "./photoLibraryModel";
import "./PhotosPage.css";

type Photo = {
    id: number;
    filename: string;
    original_name?: string | null;
    url: string;
    size_bytes: number;
    mime_type: string;
    uploaded_at?: string | null;
    taken_at?: string | null;
    owner_username?: string | null;
    source_type?: string | null;
    device_name?: string | null;
};

const PAGE_SIZE = 500;
const months = ["Tümü", "Oca", "Şub", "Mar", "Nis", "May", "Haz", "Tem", "Ağu", "Eyl", "Eki", "Kas", "Ara"];

function authHeaders(): HeadersInit {
    const token = localStorage.getItem("token");
    return token ? { Authorization: `Bearer ${token}` } : {};
}

function isVideo(photo: Photo): boolean {
    return photo.mime_type?.startsWith("video/");
}

function formatNumber(value: number): string {
    return new Intl.NumberFormat("tr-TR").format(value);
}

export default function PhotosLibraryPage() {
    const [photos, setPhotos] = useState<Photo[]>([]);
    const [total, setTotal] = useState(0);
    const [albums, setAlbums] = useState<Album[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState("");
    const [source, setSource] = useState("all");
    const [media, setMedia] = useState("all");
    const [owner, setOwner] = useState("all");
    const [query, setQuery] = useState("");
    const [year, setYear] = useState("all");
    const [month, setMonth] = useState(0);
    const [selected, setSelected] = useState<Photo | null>(null);
    const [selectionMode, setSelectionMode] = useState(false);
    const [selectedFiles, setSelectedFiles] = useState<string[]>([]);
    const [visibleLimit, setVisibleLimit] = useState(100);
    const [albumId, setAlbumId] = useState("");
    const [offset, setOffset] = useState(0);
    const [operationBusy, setOperationBusy] = useState("");
    const [operationMessage, setOperationMessage] = useState("");

    async function loadPhotos(reset = true) {
        setLoading(true);
        setError("");
        try {
            const currentOffset = reset ? 0 : offset;
            const params = new URLSearchParams({ limit: String(PAGE_SIZE), offset: String(currentOffset) });
            if (source !== "all" && source !== "other") params.set("source_type", source);
            if (media !== "all") params.set("media_type", media);
            const countParams = new URLSearchParams();
            if (source !== "all" && source !== "other") countParams.set("source_type", source);
            if (media !== "all") countParams.set("media_type", media);
            const [listResponse, countResponse] = await Promise.all([
                fetch(`${API_URL}/api/v1/photos?${params}`, { headers: authHeaders() }),
                fetch(`${API_URL}/api/v1/photos/count?${countParams}`, { headers: authHeaders() }),
            ]);
            if (!listResponse.ok || !countResponse.ok) throw new Error("Fotoğraflar alınamadı");
            const list = await listResponse.json();
            const count = await countResponse.json();
            const incoming = Array.isArray(list) ? list : [];
            setPhotos((current) => reset ? incoming : [...current, ...incoming]);
            setTotal(Number(count.count ?? 0));
            setOffset(currentOffset + incoming.length);
            if (reset) setSelectedFiles([]);
        } catch (loadError) {
            console.error(loadError);
            setError("Fotoğraflar yüklenemedi. Bağlantıyı kontrol edip yeniden deneyin.");
        } finally {
            setLoading(false);
        }
    }

    useEffect(() => {
        const timer = window.setTimeout(() => { void loadPhotos(); }, 0);
        return () => window.clearTimeout(timer);
        // loadPhotos intentionally follows the server-side source/media query state.
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [source, media]);
    useEffect(() => { void getAlbums().then((value) => setAlbums(Array.isArray(value) ? value : [])).catch(() => setAlbums([])); }, []);
    useEffect(() => {
        const onKey = (event: KeyboardEvent) => { if (event.key === "Escape") setSelected(null); };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, []);

    const owners = useMemo(() => Array.from(new Set(photos.map((photo) => photo.owner_username || "Bilinmeyen"))).sort(), [photos]);
    const baseFiltered = useMemo(() => filterPhotoCollection(photos, { owner, source, media, search: query }), [photos, owner, source, media, query]);
    const years = useMemo(() => Array.from(new Set(baseFiltered.map((photo) => getPhotoDate(photo)?.getFullYear()).filter((value): value is number => Boolean(value)))).sort((a, b) => b - a), [baseFiltered]);
    const filtered = useMemo(() => baseFiltered.filter((photo) => {
        const date = getPhotoDate(photo);
        if (year !== "all" && String(date?.getFullYear()) !== year) return false;
        return month === 0 || date?.getMonth() === month - 1;
    }), [baseFiltered, year, month]);
    const visible = filtered.slice(0, visibleLimit);
    const photoCount = photos.filter((photo) => !isVideo(photo)).length;
    const videoCount = photos.filter(isVideo).length;
    const dated = photos.map(getPhotoDate).filter((value): value is Date => Boolean(value)).sort((a, b) => a.getTime() - b.getTime());
    const dateRange = dated.length ? `${dated[0].getFullYear()}–${dated[dated.length - 1].getFullYear()}` : "—";

    function resetFilters() {
        setSource("all"); setMedia("all"); setOwner("all"); setQuery(""); setYear("all"); setMonth(0);
    }

    function toggleSelection(filename: string) {
        setSelectedFiles((current) => current.includes(filename) ? current.filter((item) => item !== filename) : [...current, filename]);
    }

    async function downloadZip() {
        if (!selectedFiles.length) return;
        setOperationBusy("zip"); setOperationMessage("");
        try {
            const response = await fetch(`${API_URL}/api/v1/photos/download-zip`, { method: "POST", headers: { "Content-Type": "application/json", ...authHeaders() }, body: JSON.stringify({ filenames: selectedFiles }) });
            if (!response.ok) throw new Error("ZIP hazırlanamadı");
            const url = URL.createObjectURL(await response.blob());
            const anchor = document.createElement("a"); anchor.href = url; anchor.download = "photoos-download.zip"; anchor.click(); URL.revokeObjectURL(url);
            setOperationMessage("ZIP indirme hazırlandı.");
        } catch (actionError) { setOperationMessage(actionError instanceof Error ? actionError.message : "ZIP indirilemedi."); }
        finally { setOperationBusy(""); }
    }

    async function addSelectionToAlbum() {
        if (!albumId || !selectedFiles.length) return;
        setOperationBusy("album"); setOperationMessage("");
        try {
            const ids = photos.filter((photo) => selectedFiles.includes(photo.filename)).map((photo) => photo.id);
            await addPhotosToAlbum(Number(albumId), ids);
            setSelectedFiles([]); setAlbumId(""); setOperationMessage(`${ids.length} öğe albüme eklendi.`);
        } catch { setOperationMessage("Albüme ekleme başarısız oldu."); }
        finally { setOperationBusy(""); }
    }

    async function deletePhoto(photo: Photo) {
        if (!window.confirm("Bu dosya kalıcı olarak silinsin mi?")) return;
        setOperationBusy("delete"); setOperationMessage("");
        try {
            const response = await fetch(`${API_URL}/api/v1/photos/file/${photo.filename}`, { method: "DELETE", headers: authHeaders() });
            if (!response.ok) throw new Error("Silme işlemi başarısız oldu.");
            setSelected(null); await loadPhotos(true); setOperationMessage("Dosya silindi.");
        } catch (actionError) { setOperationMessage(actionError instanceof Error ? actionError.message : "Dosya silinemedi."); }
        finally { setOperationBusy(""); }
    }

    function goPrevious() {
        if (!selected || !visible.length) return;
        const index = visible.findIndex((photo) => photo.filename === selected.filename);
        setSelected(visible[index <= 0 ? visible.length - 1 : index - 1]);
    }

    function goNext() {
        if (!selected || !visible.length) return;
        const index = visible.findIndex((photo) => photo.filename === selected.filename);
        setSelected(visible[(index + 1) % visible.length]);
    }

    return (
        <div className="photos-page">
            <section className="photos-hero">
                <div className="photos-hero-title"><span className="photos-title-icon">▧</span><div><h2>Fotoğraflar</h2><p>Anılarınız her zaman güvende</p></div></div>
                <label className="photos-search"><span>⌕</span><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Fotoğraflarda ara..." aria-label="Fotoğraflarda ara" /><kbd>Ctrl K</kbd></label>
                <div className="photos-actions">
                    <button className="photo-button primary" onClick={() => void loadPhotos(true)} disabled={loading}>{loading ? "Yükleniyor" : "↻ Yenile"}</button>
                    <button className={`photo-button ${selectionMode ? "active" : ""}`} onClick={() => { setSelectionMode((value) => !value); setSelectedFiles([]); }}>✓ Seç</button>
                    <select value={media} onChange={(event) => setMedia(event.target.value)} aria-label="Medya türü"><option value="all">Tüm medya</option><option value="photo">Fotoğraflar</option><option value="video">Videolar</option></select>
                    <select value={owner} onChange={(event) => setOwner(event.target.value)} aria-label="Kullanıcı"><option value="all">Tüm kullanıcılar</option>{owners.map((name) => <option key={name}>{name}</option>)}</select>
                </div>
            </section>

            <section className="photo-stat-grid" aria-label="Fotoğraf istatistikleri">
                <article className="photo-stat cyan"><span>▧</span><div><strong>{formatNumber(photoCount)}</strong><small>Fotoğraf</small></div></article>
                <article className="photo-stat violet"><span>▶</span><div><strong>{formatNumber(videoCount)}</strong><small>Video</small></div></article>
                <article className="photo-stat green"><span>▤</span><div><strong>{formatNumber(albums.length)}</strong><small>Albüm</small></div></article>
                <article className="photo-stat amber"><span>▣</span><div><strong>{dateRange}</strong><small>Zaman Aralığı</small></div></article>
            </section>

            <section className="photo-filter-panel">
                <div className="source-filter-strip" aria-label="Kaynak filtresi">
                    {SOURCE_OPTIONS.map((item) => <button key={item.value} aria-pressed={source === item.value} onClick={() => { setSource(item.value); setYear("all"); setMonth(0); setVisibleLimit(100); }}><span>{item.icon}</span>{item.label}</button>)}
                </div>
                <div className="time-filter-strip" aria-label="Yıl ve ay filtresi">
                    <div><button aria-pressed={year === "all"} onClick={() => { setYear("all"); setVisibleLimit(100); }}>Tümü</button>{years.map((item) => <button key={item} aria-pressed={year === String(item)} onClick={() => { setYear(String(item)); setVisibleLimit(100); }}>{item}</button>)}</div>
                    <div>{months.map((item, index) => <button key={item} aria-pressed={month === index} onClick={() => { setMonth(index); setVisibleLimit(100); }}>{item}</button>)}</div>
                </div>
            </section>

            <div className="photo-gallery-heading"><div><h3>{year === "all" ? "Tüm Anılar" : year}</h3><span>{formatNumber(filtered.length)} öğe · Sunucuda {formatNumber(total)}</span></div>{selectionMode && <div className="selection-toolbar"><b>Seçili: {selectedFiles.length}</b><select value={albumId} onChange={(event) => setAlbumId(event.target.value)} aria-label="Albüm seç"><option value="">Albüm seç...</option>{albums.map((album) => <option key={album.id} value={album.id}>{album.title}</option>)}</select><button onClick={() => void addSelectionToAlbum()} disabled={!albumId || !selectedFiles.length || Boolean(operationBusy)}>Albüme ekle</button><button onClick={() => void downloadZip()} disabled={!selectedFiles.length || Boolean(operationBusy)}>ZIP indir</button><button onClick={() => setSelectedFiles([])}>Temizle</button></div>}</div>
            {operationMessage && <div className="photo-operation-message" role="status">{operationMessage}</div>}

            {error ? <div className="photo-error-state"><b>Bağlantı kurulamadı</b><p>{error}</p><button onClick={() => void loadPhotos()}>Yeniden dene</button></div> : null}
            {!error && !loading && !filtered.length ? <div className="photo-empty-state"><span>◇</span><h3>Bu görünümde medya bulunamadı</h3><p>Seçili kaynak veya zaman filtresinde henüz kayıt yok.</p><button onClick={resetFilters}>Filtreleri temizle</button></div> : null}

            <section className="photo-gallery-grid" aria-label="Fotoğraf galerisi">
                {visible.map((photo) => {
                    const checked = selectedFiles.includes(photo.filename);
                    return <article className={`photo-tile ${checked ? "selected" : ""}`} key={photo.filename}>
                        <button className="photo-preview" onClick={() => selectionMode ? toggleSelection(photo.filename) : setSelected(photo)} aria-label={`${photo.original_name || photo.filename} aç`}>
                            <img src={`${API_URL}/api/v1/photos/thumb/${photo.filename}`} alt={photo.original_name || photo.filename} loading="lazy" onError={(event) => { event.currentTarget.classList.add("broken"); }} />
                            {isVideo(photo) && <span className="video-badge">▶ Video</span>}
                            <span className="photo-source-badge">{SOURCE_OPTIONS.find((item) => item.value === normalizeSourceType(photo.source_type))?.icon}</span>
                            {selectionMode && <span className="photo-check" aria-label={checked ? "Seçimi kaldır" : "Fotoğrafı seç"}>{checked ? "✓" : ""}</span>}
                        </button>
                    </article>;
                })}
            </section>
            {(visible.length < filtered.length || photos.length < total) && <button className="load-more" disabled={loading} onClick={() => { if (visible.length < filtered.length) setVisibleLimit((value) => value + 100); else void loadPhotos(false); }}>{loading ? "Yükleniyor..." : "Daha fazla göster"}</button>}

            {selected && <div className="photo-viewer" role="dialog" aria-modal="true" aria-label="Fotoğraf görüntüleyici" onClick={() => setSelected(null)}>
                <button className="viewer-close" onClick={() => setSelected(null)} aria-label="Görüntüleyiciyi kapat">×</button>
                <button className="viewer-nav previous" onClick={(event) => { event.stopPropagation(); goPrevious(); }} aria-label="Önceki fotoğraf">‹</button>
                <button className="viewer-nav next" onClick={(event) => { event.stopPropagation(); goNext(); }} aria-label="Sonraki fotoğraf">›</button>
                {isVideo(selected) ? <video controls autoPlay src={`${API_URL}/api/v1/photos/file/${selected.filename}`} onClick={(event) => event.stopPropagation()} /> : <img src={`${API_URL}/api/v1/photos/file/${selected.filename}`} alt={selected.original_name || selected.filename} onClick={(event) => event.stopPropagation()} />}
                <div className="viewer-caption"><div><b>{selected.original_name || selected.filename}</b><span>{selected.device_name || "PhotoOS"} · {normalizeSourceType(selected.source_type)}</span></div><button disabled={operationBusy === "delete"} onClick={(event) => { event.stopPropagation(); void deletePhoto(selected); }}>Sil</button></div>
            </div>}
        </div>
    );
}
