import { useCallback, useEffect, useState } from "react";
import { apiJson } from "../api/client";
import { EmptyState, ErrorBox, FeaturePage, JsonDetails, MetricCard } from "../components/FeatureShell";
import { formatDate } from "../utils/featureFormat";

type Item = { id?: string | number; level?: string; title?: string; message?: string; generated_at?: number | null; active?: boolean };
type Counts = { total?: number; active?: number; info?: number; warning?: number; error?: number; critical?: number };
type Summary = { counts?: Counts; active_notifications?: Item[] };

export default function NotificationsPage() {
    const [items, setItems] = useState<Item[]>([]);
    const [summary, setSummary] = useState<Summary>({});
    const [raw, setRaw] = useState<unknown>();
    const [error, setError] = useState("");
    const load = useCallback(async () => {
        setError("");
        try {
            const [listPayload, summaryPayload] = await Promise.all([
                apiJson<unknown>("/api/v1/notifications"),
                apiJson<Summary>("/api/v1/notifications/summary"),
            ]);
            setRaw(listPayload);
            const record = listPayload && typeof listPayload === "object" ? listPayload as Record<string, unknown> : {};
            const list = (Array.isArray(listPayload) ? listPayload : record.notifications) as Item[] | undefined;
            setItems(Array.isArray(list) ? list : []);
            setSummary(summaryPayload);
        } catch (loadError) {
            setError(loadError instanceof Error ? loadError.message : String(loadError));
        }
    }, []);
    useEffect(() => {
        const timer = window.setTimeout(() => void load(), 0);
        return () => window.clearTimeout(timer);
    }, [load]);
    const counts = summary.counts ?? {};
    return <FeaturePage title="Bildirim Merkezi" subtitle="Depolama, RAID, Backup ve sistem olayları" actions={<button className="feature-button" onClick={() => void load()}>Yenile</button>}>
        {error && <ErrorBox message={error}/>}
        <div className="metric-grid"><MetricCard label="Kritik" value={counts.critical ?? 0}/><MetricCard label="Uyarı" value={counts.warning ?? 0}/><MetricCard label="Aktif" value={counts.active ?? 0}/><MetricCard label="Toplam" value={counts.total ?? items.length}/></div>
        {items.length===0 ? <EmptyState>Gösterilecek bildirim yok.</EmptyState> : <div className="feature-grid">{items.map((n,i)=><article className="feature-card" key={String(n.id??i)}><strong>{n.title??n.level??"Bildirim"}</strong><p>{n.message??""}</p><small>{formatDate(n.generated_at)}</small></article>)}</div>}
        <JsonDetails value={raw}/>
    </FeaturePage>;
}
