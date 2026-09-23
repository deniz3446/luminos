import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { apiJson } from "../api/client";
import "./LogsPage.css";

type LogService = { key: string; title: string; units: string[] };
type LogItem = {
    id: string;
    timestamp: number;
    priority: number;
    level: string;
    unit?: string | null;
    identifier?: string | null;
    pid?: string | null;
    boot_id?: string | null;
    message: string;
};
type LogsResponse = { ok: boolean; logs?: LogItem[]; generated_at?: number };
type ServicesResponse = { ok: boolean; services?: LogService[] };
type SummaryResponse = { ok: boolean; total?: number; critical?: number; error?: number; warning?: number; info?: number; generated_at?: number };
type HealthResponse = { ok?: boolean; status?: string };

const levelLabels: Record<string, string> = { all: "Tüm seviyeler", emergency: "Acil", alert: "Alarm", critical: "Kritik", error: "Hata", warning: "Uyarı", notice: "Duyuru", info: "Bilgi", debug: "Debug", unknown: "Bilinmiyor" };
const levelIcons: Record<string, string> = { emergency: "!!", alert: "!!", critical: "!", error: "×", warning: "!", notice: "•", info: "i", debug: "⌘", unknown: "?" };
const ranges = [{ value: 15, label: "Son 15 dakika" }, { value: 60, label: "Son 1 saat" }, { value: 120, label: "Son 2 saat" }, { value: 360, label: "Son 6 saat" }, { value: 720, label: "Son 12 saat" }, { value: 1440, label: "Son 24 saat" }, { value: 10080, label: "Son 7 gün" }, { value: 43200, label: "Son 30 gün" }];
const limits = [50, 100, 200, 500, 1000];

function safeLevel(value: string) { return levelLabels[value] ? value : "unknown"; }
function formatTimestamp(value?: number) { return value ? new Intl.DateTimeFormat("tr-TR", { dateStyle: "short", timeStyle: "medium" }).format(new Date(value * 1000)) : "Bilinmiyor"; }
function formatTime(value?: number) { return value ? new Intl.DateTimeFormat("tr-TR", { hour: "2-digit", minute: "2-digit", second: "2-digit" }).format(new Date(value * 1000)) : "--:--:--"; }

export default function LogsPage() {
    const [logs, setLogs] = useState<LogItem[]>([]);
    const [services, setServices] = useState<LogService[]>([]);
    const [summary, setSummary] = useState<SummaryResponse | null>(null);
    const [service, setService] = useState("photoos");
    const [priority, setPriority] = useState("all");
    const [sinceMinutes, setSinceMinutes] = useState(120);
    const [limit, setLimit] = useState(200);
    const [search, setSearch] = useState("");
    const [debouncedSearch, setDebouncedSearch] = useState("");
    const [autoRefresh, setAutoRefresh] = useState(true);
    const [loading, setLoading] = useState(true);
    const [refreshing, setRefreshing] = useState(false);
    const [error, setError] = useState("");
    const [generatedAt, setGeneratedAt] = useState<number | null>(null);
    const [journalHealthy, setJournalHealthy] = useState<boolean | null>(null);
    const [selected, setSelected] = useState<LogItem | null>(null);
    const tableRef = useRef<HTMLDivElement | null>(null);

    useEffect(() => {
        const timer = window.setTimeout(() => setDebouncedSearch(search.trim()), 350);
        return () => window.clearTimeout(timer);
    }, [search]);

    useEffect(() => {
        void apiJson<ServicesResponse>("/api/v1/logs/services")
            .then((payload) => setServices(Array.isArray(payload.services) ? payload.services : []))
            .catch((loadError) => setError(loadError instanceof Error ? loadError.message : "Log servisleri alınamadı."));
        void apiJson<HealthResponse>("/api/v1/logs/health")
            .then((payload) => setJournalHealthy(payload.ok === true || payload.status === "healthy"))
            .catch(() => setJournalHealthy(false));
    }, []);

    const load = useCallback(async (manual = false) => {
        if (manual) setRefreshing(true);
        const params = new URLSearchParams({ service, priority, since_minutes: String(sinceMinutes), limit: String(limit) });
        if (debouncedSearch) params.set("search", debouncedSearch);
        const [logsResult, summaryResult] = await Promise.allSettled([
            apiJson<LogsResponse>(`/api/v1/logs?${params.toString()}`),
            apiJson<SummaryResponse>(`/api/v1/logs/summary?service=${encodeURIComponent(service)}&since_minutes=${sinceMinutes}&limit=1000`),
        ]);

        if (logsResult.status === "fulfilled") {
            setLogs(Array.isArray(logsResult.value.logs) ? logsResult.value.logs : []);
            setGeneratedAt(logsResult.value.generated_at ?? null);
            setError("");
        } else {
            setError(logsResult.reason instanceof Error ? logsResult.reason.message : "Log kayıtları alınamadı.");
        }
        if (summaryResult.status === "fulfilled") setSummary(summaryResult.value);
        setLoading(false);
        setRefreshing(false);
    }, [service, priority, sinceMinutes, limit, debouncedSearch]);

    useEffect(() => { const timer = window.setTimeout(() => void load(), 0); return () => window.clearTimeout(timer); }, [load]);
    useEffect(() => {
        if (!autoRefresh) return;
        const timer = window.setInterval(() => void load(), 15000);
        return () => window.clearInterval(timer);
    }, [autoRefresh, load]);

    const serviceTitle = useMemo(() => service === "all" ? "Tüm PhotoOS servisleri" : services.find((item) => item.key === service)?.title || service, [service, services]);
    const exportLogs = () => {
        const blob = new Blob([JSON.stringify({ exported_at: new Date().toISOString(), service, priority, since_minutes: sinceMinutes, search: debouncedSearch, logs }, null, 2)], { type: "application/json;charset=utf-8" });
        const href = URL.createObjectURL(blob);
        const link = document.createElement("a");
        link.href = href;
        link.download = `photoos-logs-${new Date().toISOString().replaceAll(":", "-")}.json`;
        document.body.appendChild(link); link.click(); link.remove(); URL.revokeObjectURL(href);
    };

    return (
        <main className="log-center-page">
            <header className="log-center-header">
                <div><span className="log-center-eyebrow">PhotoOS Sistem Merkezi</span><h1>Log Center</h1><p>PhotoOS servis günlüklerini gerçek zamanlı izleyin, filtreleyin ve sorunları hızlıca inceleyin.</p></div>
                <div className="log-header-actions"><button type="button" className="log-secondary-button" disabled={logs.length === 0} onClick={exportLogs}>Dışa aktar</button><button type="button" className="log-primary-button" disabled={refreshing} onClick={() => void load(true)}><span className={refreshing ? "log-spinning" : ""}>↻</span>{refreshing ? "Yenileniyor" : "Şimdi yenile"}</button></div>
            </header>

            {error && <section className="log-api-error"><strong>Log verileri alınamadı</strong><span>{error}</span></section>}

            <section className="log-summary-grid">
                <article className="log-summary-card"><span>Toplam kayıt</span><strong>{summary?.total ?? logs.length}</strong><small>{serviceTitle}</small></article>
                <article className="log-summary-card level-critical"><span>Kritik</span><strong>{summary?.critical ?? 0}</strong><small>Acil inceleme gerektirir</small></article>
                <article className="log-summary-card level-error"><span>Hata</span><strong>{summary?.error ?? 0}</strong><small>Başarısız işlemler</small></article>
                <article className="log-summary-card level-warning"><span>Uyarı</span><strong>{summary?.warning ?? 0}</strong><small>Dikkat edilmesi gereken</small></article>
                <article className="log-summary-card level-info"><span>Journal</span><strong>{journalHealthy === null ? "—" : journalHealthy ? "Hazır" : "Sorun"}</strong><small>{generatedAt ? `Son güncelleme ${formatTimestamp(generatedAt)}` : "Sistem günlük kaynağı"}</small></article>
            </section>

            <section className="log-filter-panel">
                <div className="log-filter-grid">
                    <label className="log-filter-field"><span>Servis</span><select value={service} onChange={(event) => setService(event.target.value)}><option value="all">Tüm PhotoOS servisleri</option>{services.map((item) => <option key={item.key} value={item.key}>{item.title}</option>)}</select></label>
                    <label className="log-filter-field"><span>Seviye</span><select value={priority} onChange={(event) => setPriority(event.target.value)}>{["all", "critical", "error", "warning", "notice", "info", "debug"].map((item) => <option key={item} value={item}>{levelLabels[item]}</option>)}</select></label>
                    <label className="log-filter-field"><span>Zaman aralığı</span><select value={sinceMinutes} onChange={(event) => setSinceMinutes(Number(event.target.value))}>{ranges.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}</select></label>
                    <label className="log-filter-field"><span>Kayıt sınırı</span><select value={limit} onChange={(event) => setLimit(Number(event.target.value))}>{limits.map((item) => <option key={item} value={item}>Son {item} kayıt</option>)}</select></label>
                    <label className="log-filter-field log-search-field"><span>Loglarda ara</span><input type="search" value={search} placeholder="Hata mesajı, servis veya kelime ara..." onChange={(event) => setSearch(event.target.value)} /></label>
                </div>
                <div className="log-filter-options"><label className="log-switch"><input type="checkbox" checked={autoRefresh} onChange={(event) => setAutoRefresh(event.target.checked)} /><span className="log-switch-control"/><span>Canlı yenileme<small>Her 15 saniyede</small></span></label><div className="log-last-update"><span className={autoRefresh ? "log-live-indicator" : "log-live-indicator paused"}/>{autoRefresh ? "Canlı" : "Duraklatıldı"}</div></div>
            </section>

            <section className="log-list-section">
                <div className="log-list-heading"><div><h2>Servis kayıtları</h2><p>{logs.length} kayıt gösteriliyor · {serviceTitle}</p></div>{debouncedSearch && <button type="button" className="log-clear-search" onClick={() => setSearch("")}>Aramayı temizle</button>}</div>
                <div ref={tableRef} className="log-table-container">
                    {loading && logs.length === 0 ? <div className="log-empty-state"><div className="log-loading-spinner"/><h3>Loglar yükleniyor</h3><p>Systemd journal kayıtları okunuyor.</p></div> : logs.length === 0 ? <div className="log-empty-state"><div className="log-empty-icon">✓</div><h3>Eşleşen log bulunamadı</h3><p>Seçilen filtrelerde herhangi bir kayıt yok.</p></div> : <div className="log-table"><div className="log-table-header"><span>Zaman</span><span>Seviye</span><span>Servis</span><span>Mesaj</span></div>{logs.map((item) => { const level = safeLevel(item.level); return <button key={item.id} type="button" className={`log-row level-${level}`} onClick={() => setSelected(item)}><span className="log-time-cell" title={formatTimestamp(item.timestamp)}>{formatTime(item.timestamp)}</span><span className="log-level-cell"><span className={`log-level-icon level-${level}`}>{levelIcons[level]}</span>{levelLabels[level]}</span><span className="log-service-cell" title={item.unit ?? undefined}><strong>{item.identifier || item.unit || "system"}</strong>{item.pid && <small>PID {item.pid}</small>}</span><span className="log-message-cell">{item.message || "(boş mesaj)"}</span></button>; })}</div>}
                </div>
            </section>

            {selected && <div className="log-detail-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setSelected(null); }}><section className="log-detail-panel" role="dialog" aria-modal="true" aria-label="Log detayı"><header><div><span>Log kaydı</span><h2>Detaylar</h2></div><button type="button" aria-label="Kapat" onClick={() => setSelected(null)}>×</button></header><div className="log-detail-message">{selected.message || "(boş mesaj)"}</div><dl className="log-detail-grid"><div><dt>Zaman</dt><dd>{formatTimestamp(selected.timestamp)}</dd></div><div><dt>Seviye</dt><dd>{levelLabels[safeLevel(selected.level)]}</dd></div><div><dt>Priority</dt><dd>{selected.priority}</dd></div><div><dt>Systemd unit</dt><dd>{selected.unit || "Bilinmiyor"}</dd></div><div><dt>Identifier</dt><dd>{selected.identifier || "Bilinmiyor"}</dd></div><div><dt>PID</dt><dd>{selected.pid || "Bilinmiyor"}</dd></div><div className="log-detail-full"><dt>Boot ID</dt><dd>{selected.boot_id || "Bilinmiyor"}</dd></div><div className="log-detail-full"><dt>Log ID</dt><dd>{selected.id}</dd></div></dl></section></div>}
        </main>
    );
}
