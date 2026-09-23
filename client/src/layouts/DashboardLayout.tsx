import { useEffect, useMemo, useState } from "react";
import { Navigate, NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { apiJson, clearStoredToken, getStoredToken } from "../api/client";
import "./DashboardLayout.css";

type SessionUser = { username: string; role: "admin" | "user" };
type MeEnvelope = { data?: { username?: string; role?: string }; username?: string; role?: string };
type SystemInfo = { version?: string };
type NavItem = { path: string; label: string; icon: string };
type NavGroup = { label: string; items: NavItem[] };

type PageContext = { eyebrow: string; title: string; description: string };

const adminGroups: NavGroup[] = [
    { label: "Kütüphane", items: [
        { path: "/dashboard", label: "Genel Bakış", icon: "⌂" },
        { path: "/photos", label: "Fotoğraflar", icon: "▣" },
        { path: "/albums", label: "Albümler", icon: "▤" },
        { path: "/tv", label: "TV / Medya", icon: "▶" },
        { path: "/devices", label: "Cihazlar", icon: "◇" },
    ]},
    { label: "Depolama", items: [
        { path: "/storage", label: "Depolama", icon: "▰" },
        { path: "/raid", label: "RAID", icon: "◆" },
        { path: "/backup", label: "Yedekleme", icon: "↥" },
        { path: "/pc-backups", label: "PC Backup", icon: "▱" },
    ]},
    { label: "Sistem", items: [
        { path: "/health", label: "Sağlık", icon: "♥" },
        { path: "/notifications", label: "Bildirimler", icon: "●" },
        { path: "/logs", label: "Loglar", icon: "≡" },
        { path: "/users", label: "Kullanıcılar", icon: "◉" },
        { path: "/control", label: "Kontrol", icon: "⌘" },
        { path: "/system", label: "Güncellemeler", icon: "↑" },
        { path: "/settings", label: "Ayarlar", icon: "⚙" },
    ]},
];

const userGroups: NavGroup[] = [
    { label: "Kütüphane", items: [
        { path: "/photos", label: "Fotoğraflarım", icon: "▣" },
        { path: "/albums", label: "Albümlerim", icon: "▤" },
        { path: "/tv", label: "TV / Medya", icon: "▶" },
    ]},
];

const pageContexts: Record<string, PageContext> = {
    "/dashboard": { eyebrow: "GENEL BAKIŞ", title: "Kontrol Merkezi", description: "Sistem, medya ve servislerin canlı özeti" },
    "/photos": { eyebrow: "MEDYA", title: "Fotoğraf Arşivi", description: "Fotoğraf ve videolarınızı yönetin" },
    "/albums": { eyebrow: "MEDYA", title: "Albümler", description: "Arşivinizi düzenleyin ve keşfedin" },
    "/tv": { eyebrow: "MEDYA", title: "TV / Medya", description: "Yerel ağ medya görünümü" },
    "/devices": { eyebrow: "CİHAZLAR", title: "Cihazlar", description: "PhotoOS istemcileri ve bağlantıları" },
    "/storage": { eyebrow: "DEPOLAMA", title: "Depolama", description: "Diskler ve veri alanı" },
    "/raid": { eyebrow: "DEPOLAMA", title: "RAID", description: "Disk dizisi durumu" },
    "/backup": { eyebrow: "YEDEKLEME", title: "Yedekleme", description: "PhotoOS yedekleme işlemleri" },
    "/pc-backups": { eyebrow: "YEDEKLEME", title: "PC Backup", description: "Bilgisayar yedekleri" },
    "/health": { eyebrow: "GÖZLEMLEME", title: "Sağlık Merkezi", description: "PhotoOS bileşenlerinin anlık sağlık görünümü" },
    "/notifications": { eyebrow: "GÖZLEMLEME", title: "Bildirim Merkezi", description: "Uyarılar ve sistem olayları" },
    "/logs": { eyebrow: "GÖZLEMLEME", title: "Log Center", description: "PhotoOS servis günlükleri" },
    "/users": { eyebrow: "YÖNETİM", title: "Kullanıcılar", description: "Hesap ve erişim yönetimi" },
    "/control": { eyebrow: "YÖNETİM", title: "Kontrol Merkezi", description: "Sistem, medya ve depolama görünümü" },
    "/system": { eyebrow: "SİSTEM", title: "Update Center", description: "Sürüm, paket ve güncelleme yönetimi" },
    "/settings": { eyebrow: "YÖNETİM", title: "Ayarlar", description: "PhotoOS tercihleri ve yapılandırma" },
};

function normalizeSession(payload: MeEnvelope): SessionUser {
    const data = payload.data ?? payload;
    return { username: data.username?.trim() || "Kullanıcı", role: data.role === "admin" ? "admin" : "user" };
}

function userRouteAllowed(pathname: string): boolean {
    return pathname === "/photos" || pathname === "/albums" || pathname.startsWith("/albums/") || pathname === "/tv";
}

export default function DashboardLayout() {
    const navigate = useNavigate();
    const location = useLocation();
    const hasToken = Boolean(getStoredToken());
    const [session, setSession] = useState<SessionUser | null>(null);
    const [checking, setChecking] = useState(hasToken);
    const [open, setOpen] = useState(false);
    const [serverVersion, setServerVersion] = useState("");

    useEffect(() => { const timer = window.setTimeout(() => setOpen(false), 0); return () => window.clearTimeout(timer); }, [location.pathname]);
    useEffect(() => {
        const onKey = (event: KeyboardEvent) => { if (event.key === "Escape") setOpen(false); };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, []);
    useEffect(() => {
        if (!hasToken) return;
        let active = true;
        void apiJson<MeEnvelope>("/api/v1/me")
            .then((payload) => { if (active) setSession(normalizeSession(payload)); })
            .catch(() => { if (active) { clearStoredToken(); setSession(null); } })
            .finally(() => { if (active) setChecking(false); });
        return () => { active = false; };
    }, [hasToken]);

    useEffect(() => {
        if (!session) return;
        let active = true;
        void apiJson<SystemInfo>("/api/v1/system/info")
            .then((payload) => { if (active) setServerVersion(payload.version?.trim() || ""); })
            .catch(() => { if (active) setServerVersion(""); });
        return () => { active = false; };
    }, [session]);

    const context = useMemo<PageContext>(() => {
        if (location.pathname.startsWith("/albums/")) return { eyebrow: "MEDYA", title: "Albüm", description: "Albüm içeriği" };
        return pageContexts[location.pathname] ?? { eyebrow: "PHOTOOS", title: "PhotoOS", description: "Yönetim ve fotoğraf arşivi" };
    }, [location.pathname]);

    if (!hasToken) return <Navigate to="/login" replace />;
    if (checking) return <div className="session-loading">Oturum doğrulanıyor...</div>;
    if (!session) return <Navigate to="/login" replace />;

    const isAdmin = session.role === "admin";
    if (!isAdmin && !userRouteAllowed(location.pathname)) return <Navigate to="/photos" replace />;
    const groups = isAdmin ? adminGroups : userGroups;
    const logout = () => { clearStoredToken(); navigate("/login", { replace: true }); };

    return (
        <div className="app-shell photoos-shell">
            {open && <button className="sidebar-backdrop" aria-label="Menüyü kapat" onClick={() => setOpen(false)} />}
            <aside id="photoos-sidebar" className={`sidebar photoos-sidebar ${open ? "mobile-open" : ""}`}>
                <div className="brand">
                    <div className="brand-logo" aria-hidden="true">📷</div>
                    <div><strong>PhotoOS</strong><small>{serverVersion ? `Media Server · ${serverVersion}` : "Media Server"}</small></div>
                </div>
                <nav className="nav" aria-label="PhotoOS ana menü">
                    {groups.map((group) => (
                        <div className="nav-section" key={group.label}>
                            <span className="nav-section-title">{group.label}</span>
                            {group.items.map((item) => (
                                <NavLink key={item.path} to={item.path} onClick={() => setOpen(false)}>
                                    <span className="nav-entry-icon" aria-hidden="true">{item.icon}</span>
                                    <span>{item.label}</span>
                                </NavLink>
                            ))}
                        </div>
                    ))}
                </nav>
                <button className="logout-btn" onClick={logout}>Çıkış Yap</button>
            </aside>
            <main className="main photoos-main">
                <header className="topbar photoos-topbar">
                    <button className="mobile-menu-toggle" aria-label={open ? "Menüyü kapat" : "Menüyü aç"} aria-expanded={open} aria-controls="photoos-sidebar" onClick={() => setOpen((value) => !value)}>☰</button>
                    <div className="topbar-context">
                        <span>{context.eyebrow}</span>
                        <div><h1>{context.title}</h1><p>{context.description}</p></div>
                    </div>
                    <div className="photoos-topbar-actions"><div className="user-pill">● {session.username}</div></div>
                </header>
                <section className="content photoos-content"><Outlet /></section>
            </main>
        </div>
    );
}
