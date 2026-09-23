import { useCallback, useEffect, useMemo, useState } from "react";
import type { CSSProperties } from "react";
import { useNavigate } from "react-router-dom";
import { apiJson } from "../api/client";
import "./HealthPage.css";

type Tone = "success" | "warning" | "danger" | "unknown" | "neutral";
type ProbeResult = { key: string; title: string; icon: string; path: string; route?: string; ok: boolean; payload: unknown; error?: string };
type Card = { key: string; title: string; icon: string; value: string; detail: string; tone: Tone; route?: string };

type RecordValue = Record<string, unknown>;
const isRecord = (value: unknown): value is RecordValue => typeof value === "object" && value !== null && !Array.isArray(value);
const text = (value: unknown) => typeof value === "string" ? value : typeof value === "number" || typeof value === "boolean" ? String(value) : "";
const number = (value: unknown) => typeof value === "number" && Number.isFinite(value) ? value : typeof value === "string" && value.trim() !== "" && Number.isFinite(Number(value)) ? Number(value) : null;

const probes = [
    { key: "systemHealth", title: "Sistem", icon: "🖥", path: "/api/v1/system/health" },
    { key: "systemInfo", title: "Sunucu", icon: "⌁", path: "/api/v1/system/info", route: "/control" },
    { key: "storageHealth", title: "Depolama", icon: "💾", path: "/api/v1/storage/health", route: "/storage" },
    { key: "storageDisks", title: "SMART", icon: "◉", path: "/api/v1/storage/disks", route: "/storage" },
    { key: "raid", title: "RAID", icon: "◆", path: "/api/v1/raid/status", route: "/raid" },
    { key: "backup", title: "Yedekleme", icon: "↥", path: "/api/v1/backups/status", route: "/backup" },
    { key: "notifications", title: "Bildirimler", icon: "●", path: "/api/v1/notifications/summary", route: "/notifications" },
    { key: "logs", title: "Sistem logları", icon: "≡", path: "/api/v1/logs/summary", route: "/logs" },
] as const;

function statusTone(value: string): Tone {
    const normalized = value.toLocaleLowerCase("tr-TR");
    if (["critical", "failed", "error", "degraded", "offline", "arız", "hata", "kritik"].some((item) => normalized.includes(item))) return "danger";
    if (["warning", "pending", "rebuilding", "uyarı", "bekleyen"].some((item) => normalized.includes(item))) return "warning";
    if (["healthy", "active", "running", "connected", "online", "normal", "ready", "ok", "hazır", "sağlıklı", "clean"].some((item) => normalized.includes(item))) return "success";
    return "unknown";
}

function probeCard(result: ProbeResult): Card {
    if (!result.ok) return { key: result.key, title: result.title, icon: result.icon, value: "Bilgi alınamıyor", detail: result.error || result.path, tone: "unknown", route: result.route };
    const payload = isRecord(result.payload) ? result.payload : {};

    if (result.key === "systemHealth") {
        const status = text(payload.status) || "Hazır";
        return { key: result.key, title: result.title, icon: result.icon, value: status === "healthy" ? "Sağlıklı" : status, detail: "PhotoOS API sağlık kontrolü", tone: statusTone(status), route: "/control" };
    }
    if (result.key === "systemInfo") {
        const product = text(payload.product) || "PhotoOS";
        const version = text(payload.version);
        return { key: result.key, title: result.title, icon: result.icon, value: product, detail: version ? `Sürüm ${version}` : "Sunucu bilgisi erişilebilir", tone: "success", route: result.route };
    }
    if (result.key === "storageHealth") {
        const disks = Array.isArray(payload.disks) ? payload.disks : [];
        const diskStatuses = disks.map((disk) => isRecord(disk) ? text(disk.status) : "").filter(Boolean);
        const danger = diskStatuses.filter((value) => statusTone(value) === "danger").length;
        const warning = diskStatuses.filter((value) => statusTone(value) === "warning").length;
        const tone: Tone = danger ? "danger" : warning ? "warning" : disks.length ? "success" : "unknown";
        const value = danger ? `${danger} disk kritik` : warning ? `${warning} disk uyarı` : disks.length ? `${disks.length} disk kontrol edildi` : "Disk bilgisi yok";
        return { key: result.key, title: result.title, icon: result.icon, value, detail: "SMART ve fiziksel disk sağlık özeti", tone, route: result.route };
    }
    if (result.key === "storageDisks") {
        const disks = Array.isArray(payload.disks) ? payload.disks : Array.isArray(result.payload) ? result.payload : [];
        return { key: result.key, title: result.title, icon: result.icon, value: disks.length ? `${disks.length} disk` : "Hazır", detail: "Depolama aygıt envanteri", tone: "success", route: result.route };
    }
    if (result.key === "raid") {
        const configured = payload.configured === true;
        const healthy = payload.healthy === true;
        const status = text(payload.status) || (configured ? "Durum bilinmiyor" : "Yapılandırılmamış");
        return { key: result.key, title: result.title, icon: result.icon, value: status, detail: configured ? "RAID dizi sağlık durumu" : "Bağımsız disk kullanımı", tone: configured ? (healthy ? "success" : "warning") : "neutral", route: result.route };
    }
    if (result.key === "backup") {
        const status = text(payload.status) || text(payload.state) || (payload.running === true ? "Çalışıyor" : "Hazır");
        return { key: result.key, title: result.title, icon: result.icon, value: status, detail: text(payload.last_backup) || text(payload.last_run) || "Yedekleme servisi erişilebilir", tone: statusTone(status) === "unknown" ? "success" : statusTone(status), route: result.route };
    }
    if (result.key === "notifications") {
        const counts = isRecord(payload.counts) ? payload.counts : {};
        const critical = number(counts.critical) ?? number(payload.critical) ?? 0;
        const warning = number(counts.warning) ?? number(payload.warning) ?? 0;
        const active = Array.isArray(payload.active_notifications) ? payload.active_notifications.length : 0;
        return { key: result.key, title: result.title, icon: result.icon, value: critical ? `${critical} kritik` : warning ? `${warning} uyarı` : active ? `${active} aktif` : "Temiz", detail: active ? `${active} aktif bildirim` : "Aktif kritik bildirim yok", tone: critical ? "danger" : warning ? "warning" : "success", route: result.route };
    }
    if (result.key === "logs") {
        const errors = number(payload.error) ?? 0;
        const critical = number(payload.critical) ?? 0;
        const warnings = number(payload.warning) ?? 0;
        const total = number(payload.total) ?? 0;
        return { key: result.key, title: result.title, icon: result.icon, value: critical || errors ? `${critical + errors} hata` : warnings ? `${warnings} uyarı` : "Temiz", detail: `${total} kayıt incelendi`, tone: critical || errors ? "danger" : warnings ? "warning" : "success", route: result.route };
    }

    return { key: result.key, title: result.title, icon: result.icon, value: "Hazır", detail: result.path, tone: "success", route: result.route };
}

function scoreFor(tone: Tone): number | null {
    if (tone === "success") return 100;
    if (tone === "warning") return 60;
    if (tone === "danger") return 10;
    return null;
}

export default function HealthPage() {
    const navigate = useNavigate();
    const [results, setResults] = useState<ProbeResult[]>([]);
    const [loading, setLoading] = useState(true);
    const [refreshing, setRefreshing] = useState(false);
    const [checkedAt, setCheckedAt] = useState<Date | null>(null);

    const load = useCallback(async (manual = false) => {
        if (manual) setRefreshing(true);
        const next = await Promise.all(probes.map(async (probe): Promise<ProbeResult> => {
            try {
                const payload = await apiJson<unknown>(probe.path);
                return { ...probe, ok: true, payload };
            } catch (error) {
                return { ...probe, ok: false, payload: null, error: error instanceof Error ? error.message : String(error) };
            }
        }));
        setResults(next);
        setCheckedAt(new Date());
        setLoading(false);
        setRefreshing(false);
    }, []);

    useEffect(() => {
        const initial = window.setTimeout(() => void load(), 0);
        const interval = window.setInterval(() => void load(), 20000);
        return () => { window.clearTimeout(initial); window.clearInterval(interval); };
    }, [load]);

    const cards = useMemo(() => {
        return results.map((result) => probeCard(result));
    }, [results]);
    const tones = cards.map((card) => card.tone);
    const danger = tones.filter((tone) => tone === "danger").length;
    const warning = tones.filter((tone) => tone === "warning").length;
    const success = tones.filter((tone) => tone === "success").length;
    const unknown = Math.max(0, probes.length - cards.length) + tones.filter((tone) => tone === "unknown").length;
    const scored = tones.map(scoreFor).filter((value): value is number => value !== null);
    const score = scored.length ? Math.round(scored.reduce((sum, value) => sum + value, 0) / scored.length) : null;
    const overall: Tone = danger ? "danger" : warning ? "warning" : success ? "success" : "unknown";
    const stateText = loading ? "Kontrol ediliyor" : danger ? "Müdahale gerekiyor" : warning ? "Kontrol edilmesi gerekenler var" : unknown ? "Bazı veriler kullanılamıyor" : "Tüm temel kontroller sağlıklı";

    return (
        <main className="health-center">
            <section className="health-center__hero">
                <div>
                    <span className="health-center__eyebrow">PHOTOOS HEALTH CENTER</span>
                    <h1>Sistem Sağlığı</h1>
                    <p>Donanım, depolama ve PhotoOS servislerinin merkezi sağlık görünümü.</p>
                    <div className="health-center__meta">
                        <span className={`health-center__state health-center__state--${overall}`}><i />{stateText}</span>
                        <span>Son kontrol: {checkedAt ? checkedAt.toLocaleTimeString("tr-TR") : "—"}</span>
                        <span>20 saniyede otomatik yenilenir</span>
                    </div>
                </div>
                <div className={`health-score health-score--${overall}`}>
                    <div className="health-score__ring" style={{ "--score": `${score ?? 0}` } as CSSProperties}><strong>{score ?? "—"}</strong><span>/ 100</span></div>
                    <div><strong>Sağlık puanı</strong><span>{success} sağlıklı · {warning} uyarı · {danger} kritik</span></div>
                </div>
                <button type="button" className="health-center__refresh" disabled={refreshing} onClick={() => void load(true)}><span className={refreshing ? "spinning" : ""}>↻</span>{refreshing ? "Kontrol ediliyor" : "Şimdi kontrol et"}</button>
            </section>

            {(danger > 0 || warning > 0 || unknown > 0) && (
                <section className={`health-center__notice health-center__notice--${overall}`}>
                    <div><strong>{danger ? `${danger} kritik durum algılandı` : warning ? `${warning} uyarı algılandı` : `${unknown} kaynaktan bilgi alınamıyor`}</strong><span>Ayrıntı görmek için ilgili sağlık kartını açın.</span></div>
                </section>
            )}

            <section className="health-center__section">
                <header><div><span>SİSTEM KONTROLLERİ</span><h2>Sunucu ve depolama</h2></div></header>
                <div className="health-center__grid">
                    {cards.slice(0, 5).map((card) => <HealthCard key={card.key} card={card} onOpen={() => card.route && navigate(card.route)} />)}
                </div>
            </section>

            <section className="health-center__section">
                <header><div><span>PHOTOOS SERVİSLERİ</span><h2>Servis ve olay görünümü</h2></div></header>
                <div className="health-center__grid">
                    {cards.slice(5).map((card) => <HealthCard key={card.key} card={card} onOpen={() => card.route && navigate(card.route)} />)}
                </div>
            </section>

            <section className="health-center__summary">
                <div><span className="health-center__summary-icon">✓</span><div><strong>{success}</strong><span>Sağlıklı kontrol</span></div></div>
                <div><span className="health-center__summary-icon">!</span><div><strong>{warning}</strong><span>Uyarı</span></div></div>
                <div><span className="health-center__summary-icon">×</span><div><strong>{danger}</strong><span>Kritik durum</span></div></div>
                <div><span className="health-center__summary-icon">?</span><div><strong>{unknown}</strong><span>Bilgi alınamayan</span></div></div>
            </section>
        </main>
    );
}

function HealthCard({ card, onOpen }: { card: Card; onOpen: () => void }) {
    return (
        <button type="button" className={`health-item health-item--${card.tone}`} disabled={!card.route} onClick={onOpen}>
            <div className="health-item__top"><span className="health-item__icon">{card.icon}</span><span className="health-item__indicator" /></div>
            <span className="health-item__title">{card.title}</span>
            <strong className="health-item__value">{card.value}</strong>
            <span className="health-item__detail">{card.detail}</span>
            {card.route && <span className="health-item__link">Ayrıntıları aç <span>→</span></span>}
        </button>
    );
}
