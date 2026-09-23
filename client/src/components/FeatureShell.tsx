import type { ReactNode } from "react";
import "../pages/FeaturePages.css";

export function FeaturePage({ title, subtitle, actions, children }: { title: string; subtitle?: string; actions?: ReactNode; children: ReactNode }) {
    return <section className="feature-page">
        <header className="feature-header">
            <div><h1>{title}</h1>{subtitle && <p>{subtitle}</p>}</div>
            {actions && <div className="feature-actions">{actions}</div>}
        </header>
        {children}
    </section>;
}

export function MetricCard({ label, value, hint }: { label: string; value: ReactNode; hint?: ReactNode }) {
    return <article className="metric-card"><span>{label}</span><strong>{value}</strong>{hint && <small>{hint}</small>}</article>;
}

export function ErrorBox({ message }: { message: string }) { return <div className="feature-error" role="alert">{message}</div>; }
export function EmptyState({ children }: { children: ReactNode }) { return <div className="feature-empty">{children}</div>; }
export function StatusPill({ ok, label }: { ok: boolean; label?: string }) { return <span className={`status-pill ${ok ? "ok" : "bad"}`}>{label ?? (ok ? "Sağlıklı" : "Sorun")}</span>; }

export function JsonDetails({ value }: { value: unknown }) {
    return <details className="json-details"><summary>Ham yanıt</summary><pre>{JSON.stringify(value, null, 2)}</pre></details>;
}

