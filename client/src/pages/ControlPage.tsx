import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { apiJson } from "../api/client";
import "./ControlPage.css";

type StorageInfo = {
    total_bytes: number;
    used_bytes: number;
    free_bytes: number;
    usage_percent?: number;
};

type SystemInfo = {
    product?: string;
    version?: string;
    status?: string;
    api?: string;
};

type HealthInfo = { status?: string };

type DashboardStats = {
    user_count: number;
    total_media_count: number;
    photo_count: number;
    video_count: number;
    version?: string;
};

type Photo = {
    id: number;
    mime_type?: string;
    uploaded_at?: string;
};

type ControlState = {
    system: SystemInfo | null;
    health: HealthInfo | null;
    storage: StorageInfo | null;
    photos: Photo[];
    stats: DashboardStats | null;
};

const emptyState: ControlState = {
    system: null,
    health: null,
    storage: null,
    photos: [],
    stats: null,
};

function formatBytes(bytes?: number) {
    if (!bytes || bytes < 0) return "0 GB";
    return `${(bytes / 1024 / 1024 / 1024).toFixed(bytes >= 1024 ** 4 ? 0 : 1)} GB`;
}

function formatDate(value?: string) {
    if (!value) return "Henüz yükleme yok";
    const normalized = value.includes("T") ? value : value.replace(" ", "T");
    const date = new Date(normalized);
    return Number.isNaN(date.getTime()) ? value : date.toLocaleString("tr-TR");
}

function settledValue<T>(result: PromiseSettledResult<T>): T | null {
    return result.status === "fulfilled" ? result.value : null;
}

export default function ControlPage() {
    const [data, setData] = useState<ControlState>(emptyState);
    const [loading, setLoading] = useState(true);
    const [lastChecked, setLastChecked] = useState("");
    const [unavailable, setUnavailable] = useState<string[]>([]);

    const loadControl = useCallback(async () => {
        setLoading(true);
        const stamp = Date.now();
        const results = await Promise.allSettled([
            apiJson<SystemInfo>(`/api/v1/system/info?ts=${stamp}`),
            apiJson<HealthInfo>(`/api/v1/system/health?ts=${stamp}`),
            apiJson<StorageInfo>(`/api/v1/storage?ts=${stamp}`),
            apiJson<Photo[]>(`/api/v1/photos?limit=20&offset=0&ts=${stamp}`),
            apiJson<DashboardStats>(`/api/v1/dashboard/stats?ts=${stamp}`),
        ] as const);

        const [systemResult, healthResult, storageResult, photosResult, statsResult] = results;
        const nextUnavailable: string[] = [];
        if (systemResult.status === "rejected") nextUnavailable.push("Sistem bilgisi");
        if (healthResult.status === "rejected") nextUnavailable.push("API sağlığı");
        if (storageResult.status === "rejected") nextUnavailable.push("Depolama");
        if (photosResult.status === "rejected") nextUnavailable.push("Son medya");
        if (statsResult.status === "rejected") nextUnavailable.push("Medya istatistikleri");

        const photos = settledValue(photosResult);
        setData({
            system: settledValue(systemResult),
            health: settledValue(healthResult),
            storage: settledValue(storageResult),
            photos: Array.isArray(photos) ? photos : [],
            stats: settledValue(statsResult),
        });
        setUnavailable(nextUnavailable);
        setLastChecked(new Date().toLocaleTimeString("tr-TR"));
        setLoading(false);
    }, []);

    useEffect(() => {
        void loadControl();
        const timer = window.setInterval(() => void loadControl(), 15000);
        return () => window.clearInterval(timer);
    }, [loadControl]);

    const healthy = data.health?.status?.toLowerCase() === "healthy";
    const apiStatus = data.health ? (healthy ? "Sağlıklı" : data.health.status || "Uyarı") : "Veri alınamıyor";
    const usage = data.storage
        ? Math.max(0, Math.min(100, data.storage.usage_percent ?? (data.storage.total_bytes ? data.storage.used_bytes / data.storage.total_bytes * 100 : 0)))
        : 0;

    const photoFallback = useMemo(
        () => data.photos.filter((item) => !item.mime_type?.startsWith("video/")).length,
        [data.photos],
    );
    const videoFallback = useMemo(
        () => data.photos.filter((item) => item.mime_type?.startsWith("video/")).length,
        [data.photos],
    );
    const latestUpload = useMemo(() => {
        return data.photos
            .map((item) => item.uploaded_at)
            .filter((value): value is string => Boolean(value))
            .sort((a, b) => new Date(b.replace(" ", "T")).getTime() - new Date(a.replace(" ", "T")).getTime())[0];
    }, [data.photos]);

    return (
        <div className="control-center-page">
            <section className="control-hero">
                <div>
                    <span className="control-eyebrow">CANLI SİSTEM ÖZETİ</span>
                    <h2>PhotoOS Kontrol Merkezi</h2>
                    <p>Servis, medya ve depolama durumunu tek ekranda izleyin.</p>
                </div>
                <div className="control-refresh-block">
                    <span>Son kontrol {lastChecked || "—"}</span>
                    <button type="button" onClick={() => void loadControl()} disabled={loading}>
                        {loading ? "Kontrol ediliyor…" : "Şimdi yenile"}
                    </button>
                </div>
            </section>

            {unavailable.length > 0 && (
                <div className="control-notice" role="status">
                    <strong>Bazı veriler alınamadı.</strong>
                    <span>{unavailable.join(" · ")}</span>
                </div>
            )}

            <section className="control-summary-grid" aria-label="Sistem özeti">
                <SummaryCard label="API Durumu" value={apiStatus} detail={healthy ? "İsteklere yanıt veriyor" : "Sağlık kontrolünü inceleyin"} tone={healthy ? "good" : "warn"} />
                <SummaryCard label="Sistem" value={data.system?.product || "Veri alınamıyor"} detail={data.system?.status || "Durum bilgisi yok"} />
                <SummaryCard label="Sürüm" value={data.system?.version || "Veri alınamıyor"} detail={data.system?.api ? `API ${data.system.api}` : "Sürüm bilgisi yok"} />
                <SummaryCard label="Toplam Medya" value={String(data.stats?.total_media_count ?? data.photos.length)} detail={`${data.stats?.photo_count ?? photoFallback} fotoğraf · ${data.stats?.video_count ?? videoFallback} video`} />
            </section>

            <div className="control-grid">
                <section className="control-panel control-storage-panel">
                    <div className="control-panel-heading">
                        <div><span>DEPOLAMA</span><h3>Alan kullanımı</h3></div>
                        <Link to="/storage">Detayları aç →</Link>
                    </div>
                    {data.storage ? (
                        <>
                            <div className="control-storage-meter" aria-label={`Depolama yüzde ${Math.round(usage)} dolu`}>
                                <div style={{ width: `${usage}%` }} />
                            </div>
                            <div className="control-storage-percent">%{usage.toFixed(1)} kullanılıyor</div>
                            <dl className="control-definition-grid">
                                <div><dt>Toplam</dt><dd>{formatBytes(data.storage.total_bytes)}</dd></div>
                                <div><dt>Kullanılan</dt><dd>{formatBytes(data.storage.used_bytes)}</dd></div>
                                <div><dt>Boş</dt><dd>{formatBytes(data.storage.free_bytes)}</dd></div>
                            </dl>
                        </>
                    ) : <EmptyPanel text="Depolama verisi alınamıyor." />}
                </section>

                <section className="control-panel">
                    <div className="control-panel-heading">
                        <div><span>MEDYA</span><h3>Kütüphane özeti</h3></div>
                        <Link to="/photos">Arşivi aç →</Link>
                    </div>
                    <dl className="control-definition-grid control-media-grid">
                        <div><dt>Fotoğraflar</dt><dd>{data.stats?.photo_count ?? photoFallback}</dd></div>
                        <div><dt>Videolar</dt><dd>{data.stats?.video_count ?? videoFallback}</dd></div>
                        <div><dt>Kullanıcılar</dt><dd>{data.stats?.user_count ?? "—"}</dd></div>
                        <div><dt>Son yükleme</dt><dd className="control-date-value">{formatDate(latestUpload)}</dd></div>
                    </dl>
                </section>
            </div>

            <section className="control-panel">
                <div className="control-panel-heading">
                    <div><span>YÖNETİM</span><h3>Hızlı geçişler</h3></div>
                    <p>Yalnızca etkin PhotoOS bileşenleri</p>
                </div>
                <div className="control-links">
                    <QuickLink to="/health" title="Sağlık Merkezi" description="Disk, RAID, yedekleme ve servis sağlığı" />
                    <QuickLink to="/logs" title="Log Center" description="Servis günlükleri ve hata inceleme" />
                    <QuickLink to="/system" title="Update Center" description="Sürüm ve paket yönetimi" />
                    <QuickLink to="/notifications" title="Bildirimler" description="Aktif uyarılar ve olaylar" />
                </div>
            </section>
        </div>
    );
}

function SummaryCard({ label, value, detail, tone = "neutral" }: { label: string; value: string; detail: string; tone?: "neutral" | "good" | "warn" }) {
    return <article className={`control-status control-status-${tone}`}><span>{label}</span><strong>{value}</strong><small>{detail}</small></article>;
}

function EmptyPanel({ text }: { text: string }) {
    return <div className="control-empty"><strong>Veri alınamıyor</strong><span>{text}</span></div>;
}

function QuickLink({ to, title, description }: { to: string; title: string; description: string }) {
    return <Link className="control-link-card" to={to}><strong>{title}</strong><span>{description}</span><b>→</b></Link>;
}
