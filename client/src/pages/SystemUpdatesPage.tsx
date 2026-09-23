import { useCallback, useEffect, useMemo, useState } from "react";
import { api, apiJson } from "../api/client";
import "./SystemUpdatesPage.css";

type AgentStatus = {
    product?: string;
    agent_version?: string;
    mode?: string;
    timestamp?: string;
    healthy?: boolean;
    health?: {
        healthy?: boolean;
        server_http?: boolean;
        web_http?: boolean;
        database_exists?: boolean;
        storage_exists?: boolean;
        server_service?: { state?: string; active?: boolean };
        web_service?: { state?: string; active?: boolean };
        active_release?: { version?: string | null; path?: string | null; valid?: boolean };
    };
};

type Release = {
    version: string;
    path: string;
    active: boolean;
    server_exists?: boolean;
    config_exists?: boolean;
    migrations_exist?: boolean;
    web_exists?: boolean;
};

type ReleasesResponse = {
    active_release?: { version?: string | null; path?: string | null; valid?: boolean };
    releases?: Release[];
    timestamp?: string;
};

type PackageVerification = {
    valid?: boolean;
    version?: string;
    verified_at?: string;
    verified_files?: number;
    package_sha256?: string;
    manifest?: { version?: string; release_notes?: string; channel?: string };
};

type UpdatePackage = {
    filename: string;
    size_bytes: number;
    sha256?: string;
    verified?: boolean;
    verification?: PackageVerification | null;
};

type PackagesResponse = { packages?: UpdatePackage[]; timestamp?: string };
type HistoryEntry = { filename: string; payload: Record<string, unknown> };
type HistoryResponse = { history?: HistoryEntry[]; timestamp?: string };
type TabName = "overview" | "packages" | "releases" | "history";

function formatBytes(bytes?: number) {
    if (!bytes || !Number.isFinite(bytes) || bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let value = bytes;
    let index = 0;
    while (value >= 1024 && index < units.length - 1) {
        value /= 1024;
        index += 1;
    }
    return `${value.toLocaleString("tr-TR", { maximumFractionDigits: 2 })} ${units[index]}`;
}

function formatDate(value?: string) {
    if (!value) return "—";
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? value : date.toLocaleString("tr-TR");
}

function packageVersion(item: UpdatePackage) {
    return item.verification?.version || item.verification?.manifest?.version || item.filename;
}

async function actionError(response: Response) {
    const payload = await response.json().catch(() => ({} as Record<string, unknown>));
    const message = typeof payload.message === "string" ? payload.message : typeof payload.error === "string" ? payload.error : `HTTP ${response.status}`;
    return new Error(message);
}


/* PHOTOOS_SIGNED_UPDATE_UI */

type SignedUpdateChannel = "stable" | "beta" | "disabled";

type SignedUpdateCheck = {
    channel: SignedUpdateChannel;
    running_version: string;
    update_agent_version: string;
    status: "disabled" | "up_to_date" | "incompatible" | "replay_rejected" | "update_available";
    update_available: boolean;
    version?: string;
    release_id?: string;
    package_filename?: string;
};

type SignedUpdateTarget = {
    version: string;
    release_id: string;
    package_filename: string;
};

type SignedStagedUpdate = SignedUpdateTarget & {
    status: "staged";
    size: number;
    sha256: string;
};

type SignedVerifiedUpdate = SignedUpdateTarget & {
    status: "verified";
    operation_id: string;
    size: number;
    sha256: string;
};

type SignedInstallResult = {
    status: "installing";
    operation_id: string;
    version: string;
    release_id: string;
    package_filename: string;
};

async function signedUpdateRequest<T>(
    path: string,
    init: RequestInit = {},
): Promise<T> {
    const headers = new Headers(init.headers);

    if (init.body !== undefined && !headers.has("Content-Type")) {
        headers.set("Content-Type", "application/json");
    }

    const response = await api(path, {
        ...init,
        headers,
    });

    if (!response.ok) {
        throw await actionError(response);
    }

    return await response.json() as T;
}

export default function SystemUpdatesPage() {
    const [tab, setTab] = useState<TabName>("overview");
    const [status, setStatus] = useState<AgentStatus | null>(null);
    const [releases, setReleases] = useState<ReleasesResponse | null>(null);
    const [packages, setPackages] = useState<PackagesResponse | null>(null);
    const [history, setHistory] = useState<HistoryResponse | null>(null);
    const [unavailable, setUnavailable] = useState<string[]>([]);
    const [loading, setLoading] = useState(true);
    const [refreshing, setRefreshing] = useState(false);
    const [action, setAction] = useState("");
    const [selectedPackage, setSelectedPackage] = useState<File | null>(null);
    const [message, setMessage] = useState("");
    const [error, setError] = useState("");
    const [lastUpdated, setLastUpdated] = useState("");

    const loadData = useCallback(async (quiet = false) => {
        quiet ? setRefreshing(true) : setLoading(true);
        const stamp = Date.now();
        const results = await Promise.allSettled([
            apiJson<AgentStatus>(`/api/v1/system/update/status?ts=${stamp}`),
            apiJson<ReleasesResponse>(`/api/v1/system/update/releases?ts=${stamp}`),
            apiJson<PackagesResponse>(`/api/v1/system/update/packages?ts=${stamp}`),
            apiJson<HistoryResponse>(`/api/v1/system/update/history?ts=${stamp}`),
        ] as const);
        const [statusResult, releasesResult, packagesResult, historyResult] = results;
        const missing: string[] = [];
        if (statusResult.status === "fulfilled") setStatus(statusResult.value); else { setStatus(null); missing.push("Update Agent"); }
        if (releasesResult.status === "fulfilled") setReleases(releasesResult.value); else { setReleases(null); missing.push("Release listesi"); }
        if (packagesResult.status === "fulfilled") setPackages(packagesResult.value); else { setPackages(null); missing.push("Paket listesi"); }
        if (historyResult.status === "fulfilled") setHistory(historyResult.value); else { setHistory(null); missing.push("Güncelleme geçmişi"); }
        setUnavailable(missing);
        setLastUpdated(new Date().toLocaleTimeString("tr-TR"));
        setLoading(false);
        setRefreshing(false);
    }, []);

    useEffect(() => {
        void loadData();
        const timer = window.setInterval(() => void loadData(true), 15000);
        return () => window.clearInterval(timer);
    }, [loadData]);

    const packageList = packages?.packages ?? [];
    const verifiedPackages = useMemo(() => packageList.filter((item) => item.verified || item.verification?.valid), [packageList]);
    const activeVersion = releases?.active_release?.version || status?.health?.active_release?.version || null;
    const agentHealthy = Boolean(status?.healthy ?? status?.health?.healthy);

    async function postAction(url: string, body: Record<string, string>, key: string, success: string, delayedRefresh = false) {
        try {
            setAction(key);
            setError("");
            setMessage("");
            const response = await api(url, { method: "POST", body: JSON.stringify(body) });
            if (!response.ok) throw await actionError(response);
            setMessage(success);
            if (delayedRefresh) {
                window.setTimeout(() => void loadData(true), 7000);
                window.setTimeout(() => void loadData(true), 16000);
            } else {
                await loadData(true);
            }
        } catch (caught) {
            setError(caught instanceof Error ? caught.message : "İşlem başarısız oldu.");
        } finally {
            setAction("");
        }
    }

    async function uploadPackage() {
        if (!selectedPackage) return setError("Önce bir .popkg dosyası seçin.");
        if (!selectedPackage.name.toLowerCase().endsWith(".popkg")) return setError("Yalnızca .popkg dosyaları yüklenebilir.");
        if (selectedPackage.size <= 0 || selectedPackage.size > 512 * 1024 * 1024) return setError("Paket boş veya 512 MB sınırını aşıyor.");
        try {
            setAction("upload");
            setError("");
            setMessage("");
            const form = new FormData();
            form.append("file", selectedPackage, selectedPackage.name);
            const response = await api("/api/v1/system/update/upload", { method: "POST", body: form });
            if (!response.ok) throw await actionError(response);
            setMessage("Paket sunucuya yüklendi. Kurulumdan önce doğrulayın.");
            setSelectedPackage(null);
            await loadData(true);
        } catch (caught) {
            setError(caught instanceof Error ? caught.message : "Paket yüklenemedi.");
        } finally {
            setAction("");
        }
    }

    const verifyPackage = (filename: string) => postAction("/api/v1/system/update/verify", { filename }, `verify:${filename}`, "Paket doğrulandı.");
    const installPackage = async (filename: string) => {
        const item = packageList.find((entry) => entry.filename === filename);
        if (!window.confirm(`${packageVersion(item ?? { filename, size_bytes: 0 })} kurulacak. PhotoOS servisleri kısa süreliğine yeniden başlayabilir. Devam edilsin mi?`)) return;
        await postAction("/api/v1/system/update/install", { filename }, `install:${filename}`, "Güncelleme isteği kabul edildi. Servislerin dönmesi bekleniyor.", true);
    };
    const rollback = async (version: string) => {
        if (!window.confirm(`${version} sürümüne geri dönüş isteği gönderilecek. Devam edilsin mi?`)) return;
        await postAction("/api/v1/system/update/rollback", { version }, `rollback:${version}`, "Geri dönüş isteği kabul edildi. Servislerin dönmesi bekleniyor.", true);
    };

    if (loading) {
        return <div className="update-center-page"><div className="uc-loading"><div className="uc-loading-spinner" /><strong>Update Center hazırlanıyor</strong><p>Agent, release ve paket bilgileri kontrol ediliyor.</p></div></div>;
    }

    return (
        <div className="update-center-page">
            <section className="uc-hero">
                <div className="uc-hero-main"><div className="uc-hero-icon">↑</div><div><span className="uc-eyebrow">PHOTOOS UPDATE CENTER</span><h2>Sistem Güncellemeleri</h2><p>Release, paket ve güncelleme geçmişini güvenli bir akışta yönetin.</p></div></div>
                <div className="uc-hero-actions"><span className={`uc-status-pill ${agentHealthy ? "ok" : "bad"}`}><i />{status ? (agentHealthy ? "Agent sağlıklı" : "Agent uyarı veriyor") : "Update Agent erişilemiyor"}</span><button className="uc-refresh-button" type="button" disabled={refreshing} onClick={() => void loadData(true)}>{refreshing ? "Yenileniyor…" : "Yenile"}</button></div>
            </section>

            {unavailable.length > 0 && <div className="uc-alert warning"><strong>Update Center kısmen erişilebilir.</strong><span>{unavailable.join(" · ")}</span></div>}
            {error && <div className="uc-alert error"><strong>İşlem tamamlanamadı.</strong><span>{error}</span><button type="button" onClick={() => setError("")}>×</button></div>}
            {message && <div className="uc-alert success"><strong>İşlem kabul edildi.</strong><span>{message}</span><button type="button" onClick={() => setMessage("")}>×</button></div>}

            <section className="uc-summary-grid">
                <Summary icon="●" label="Update Agent" value={status ? (agentHealthy ? "Sağlıklı" : "Uyarı") : "Erişilemiyor"} detail={status?.agent_version ? `Agent ${status.agent_version}` : "Durum bilgisi yok"} state={agentHealthy ? "ok" : "warn"} />
                <Summary icon="◆" label="Aktif Release" value={activeVersion || "Veri alınamıyor"} detail={releases?.active_release?.valid === false ? "Release doğrulaması başarısız" : "Çalışan PhotoOS sürümü"} />
                <Summary icon="▣" label="Paketler" value={packages ? String(packageList.length) : "—"} detail={`${verifiedPackages.length} doğrulanmış paket`} />
                <Summary icon="◷" label="Son Kontrol" value={lastUpdated || "—"} detail={status?.timestamp ? formatDate(status.timestamp) : "Yerel kontrol zamanı"} />
            </section>

            <nav className="uc-tabs" aria-label="Update Center bölümleri">
                {([['overview','Genel Bakış'],['packages','Paketler'],['releases','Release’ler'],['history','Geçmiş']] as Array<[TabName,string]>).map(([value,label]) => <button key={value} type="button" className={tab === value ? "active" : ""} onClick={() => setTab(value)}>{label}</button>)}
            </nav>

            {tab === "overview" && <Overview status={status} releases={releases} packages={packages} />}
            {tab === "packages" && <PackagesPanel packages={packages} selectedPackage={selectedPackage} setSelectedPackage={setSelectedPackage} action={action} uploadPackage={uploadPackage} verifyPackage={verifyPackage} installPackage={installPackage} />}
            {tab === "releases" && <ReleasesPanel releases={releases} action={action} rollback={rollback} />}
            {tab === "history" && <HistoryPanel history={history} />}
        </div>
    );
}

function Summary({ icon, label, value, detail, state = "neutral" }: { icon: string; label: string; value: string; detail: string; state?: "neutral" | "ok" | "warn" }) {
    return <article className={`uc-summary-card ${state}`}><div className="uc-summary-icon">{icon}</div><div className="uc-summary-content"><span>{label}</span><strong>{value}</strong><small>{detail}</small></div></article>;
}


function SignedUpdatePanel({
    status,
    releases,
}: {
    status: AgentStatus | null;
    releases: ReleasesResponse | null;
}) {
    const [channel, setChannel] =
        useState<SignedUpdateChannel>("stable");

    const [check, setCheck] =
        useState<SignedUpdateCheck | null>(null);

    const [staged, setStaged] =
        useState<SignedStagedUpdate | null>(null);

    const [verified, setVerified] =
        useState<SignedVerifiedUpdate | null>(null);

    const [busy, setBusy] = useState("");
    const [notice, setNotice] = useState("");
    const [signedError, setSignedError] = useState("");

    useEffect(() => {
        let cancelled = false;

        void signedUpdateRequest<{
            channel: SignedUpdateChannel;
        }>("/api/v1/system/update/channel")
            .then((result) => {
                if (!cancelled) setChannel(result.channel);
            })
            .catch(() => {});

        return () => {
            cancelled = true;
        };
    }, []);

    const target: SignedUpdateTarget | null =
        check?.update_available &&
        check.version &&
        check.release_id &&
        check.package_filename
            ? {
                  version: check.version,
                  release_id: check.release_id,
                  package_filename: check.package_filename,
              }
            : null;

    async function saveChannel(next: SignedUpdateChannel) {
        try {
            setBusy("channel");
            setSignedError("");
            setNotice("");

            const result = await signedUpdateRequest<{
                channel: SignedUpdateChannel;
                saved: boolean;
            }>("/api/v1/system/update/channel", {
                method: "POST",
                body: JSON.stringify({ channel: next }),
            });

            setChannel(result.channel);
            setCheck(null);
            setStaged(null);
            setVerified(null);
            setNotice(
                result.channel === "disabled"
                    ? "?evrimi?i g?ncellemeler devre d???."
                    : `G?ncelleme kanal? ${result.channel} olarak kaydedildi.`,
            );
        } catch (caught) {
            setSignedError(
                caught instanceof Error
                    ? caught.message
                    : "G?ncelleme kanal? kaydedilemedi.",
            );
        } finally {
            setBusy("");
        }
    }

    async function checkSignedUpdate() {
        try {
            setBusy("check");
            setSignedError("");
            setNotice("");
            setStaged(null);
            setVerified(null);

            const result =
                await signedUpdateRequest<SignedUpdateCheck>(
                    "/api/v1/system/update/check",
                    { method: "POST" },
                );

            setCheck(result);
            setChannel(result.channel);

            if (result.status === "update_available") {
                setNotice(`PhotoOS ${result.version} g?ncellemesi haz?r.`);
            } else if (result.status === "up_to_date") {
                setNotice("PhotoOS g?ncel.");
            } else if (result.status === "disabled") {
                setNotice("?evrimi?i g?ncellemeler devre d???.");
            } else if (result.status === "incompatible") {
                setNotice("Bulunan g?ncelleme bu sistemle uyumlu de?il.");
            } else {
                setNotice("G?ncelleme bildirimi g?venlik nedeniyle reddedildi.");
            }
        } catch (caught) {
            setSignedError(
                caught instanceof Error
                    ? caught.message
                    : "G?ncelleme kontrol? ba?ar?s?z.",
            );
        } finally {
            setBusy("");
        }
    }

    async function downloadSignedUpdate() {
        if (!target) return;

        try {
            setBusy("download");
            setSignedError("");

            const result =
                await signedUpdateRequest<SignedStagedUpdate>(
                    "/api/v1/system/update/download",
                    {
                        method: "POST",
                        body: JSON.stringify(target),
                    },
                );

            setStaged(result);
            setVerified(null);
            setNotice(`${result.package_filename} indirildi.`);
        } catch (caught) {
            setSignedError(
                caught instanceof Error
                    ? caught.message
                    : "G?ncelleme indirilemedi.",
            );
        } finally {
            setBusy("");
        }
    }

    async function verifySignedUpdate() {
        if (!target || !staged) return;

        try {
            setBusy("verify");
            setSignedError("");

            const result =
                await signedUpdateRequest<SignedVerifiedUpdate>(
                    "/api/v1/system/update/verify-staged",
                    {
                        method: "POST",
                        body: JSON.stringify(target),
                    },
                );

            setVerified(result);
            setNotice("Paket g?venli bi?imde do?ruland?.");
        } catch (caught) {
            setSignedError(
                caught instanceof Error
                    ? caught.message
                    : "Paket do?rulanamad?.",
            );
        } finally {
            setBusy("");
        }
    }

    async function installSignedUpdate() {
        if (!verified) return;

        const approved = window.confirm(
            `PhotoOS ${verified.version} kurulacak.\n\n` +
            "Paket imzas? ve kimli?i do?ruland?. " +
            "Kurulum servisleri yeniden ba?latabilir.\n\n" +
            "Kurulumu onayl?yor musunuz?",
        );

        if (!approved) return;

        try {
            setBusy("install");
            setSignedError("");

            const result =
                await signedUpdateRequest<SignedInstallResult>(
                    "/api/v1/system/update/install-verified",
                    {
                        method: "POST",
                        body: JSON.stringify({
                            operation_id: verified.operation_id,
                            approved: true,
                        }),
                    },
                );

            setNotice(
                `PhotoOS ${result.version} kurulumu ba?lat?ld?.`,
            );
        } catch (caught) {
            setSignedError(
                caught instanceof Error
                    ? caught.message
                    : "Kurulum ba?lat?lamad?.",
            );
        } finally {
            setBusy("");
        }
    }

    const activeVersion =
        releases?.active_release?.version ||
        status?.health?.active_release?.version ||
        "?";

    return (
        <section className="uc-panel uc-signed-update-panel">
            <div className="uc-panel-heading">
                <div>
                    <span className="uc-panel-kicker">
                        ?MZALI ?EVR?M??? G?NCELLEME
                    </span>
                    <h3>G?venli PhotoOS g?ncellemesi</h3>
                </div>
            </div>

            <div className="uc-signed-toolbar">
                <label>
                    <span>G?ncelleme kanal?</span>
                    <select
                        value={channel}
                        disabled={Boolean(busy)}
                        onChange={(event) =>
                            void saveChannel(
                                event.target.value as SignedUpdateChannel,
                            )
                        }
                    >
                        <option value="stable">Stable</option>
                        <option value="beta">Beta</option>
                        <option value="disabled">Devre d???</option>
                    </select>
                </label>

                <button
                    type="button"
                    className="uc-primary-button"
                    disabled={Boolean(busy)}
                    onClick={() => void checkSignedUpdate()}
                >
                    {busy === "check"
                        ? "Kontrol ediliyor?"
                        : "G?ncellemeyi kontrol et"}
                </button>
            </div>

            {signedError && (
                <div className="uc-alert error">
                    <strong>G?ncelleme hatas?</strong>
                    <span>{signedError}</span>
                </div>
            )}

            {notice && (
                <div className="uc-alert success">
                    <strong>G?ncelleme durumu</strong>
                    <span>{notice}</span>
                </div>
            )}

            <div className="uc-signed-grid">
                <div>
                    <span>?al??an s?r?m</span>
                    <strong>{check?.running_version || activeVersion}</strong>
                </div>
                <div>
                    <span>Bulunan s?r?m</span>
                    <strong>{check?.version || "?"}</strong>
                </div>
                <div>
                    <span>Update Agent</span>
                    <strong>{check?.update_agent_version || status?.agent_version || "?"}</strong>
                </div>
                <div>
                    <span>Paket</span>
                    <strong>
                        {verified?.package_filename ||
                            staged?.package_filename ||
                            check?.package_filename ||
                            "?"}
                    </strong>
                </div>
            </div>

            {staged && (
                <div className="uc-signed-identity">
                    <span>{formatBytes(staged.size)}</span>
                    <code>SHA256 {staged.sha256}</code>
                </div>
            )}

            <div className="uc-signed-actions">
                <button
                    type="button"
                    disabled={!target || Boolean(busy) || Boolean(staged)}
                    onClick={() => void downloadSignedUpdate()}
                >
                    {busy === "download"
                        ? "?ndiriliyor?"
                        : staged
                          ? "1. ?ndirildi"
                          : "1. ?ndir"}
                </button>

                <button
                    type="button"
                    disabled={!staged || Boolean(busy) || Boolean(verified)}
                    onClick={() => void verifySignedUpdate()}
                >
                    {busy === "verify"
                        ? "Do?rulan?yor?"
                        : verified
                          ? "2. Do?ruland?"
                          : "2. Do?rula"}
                </button>

                <button
                    type="button"
                    className="primary"
                    disabled={!verified || Boolean(busy)}
                    onClick={() => void installSignedUpdate()}
                >
                    {busy === "install"
                        ? "Kurulum ba?lat?l?yor?"
                        : "3. Kur"}
                </button>
            </div>

            <small className="uc-signed-note">
                Kurulum yaln?z do?rulanm?? operation kimli?i
                ve a??k kullan?c? onay?yla ba?lat?l?r.
            </small>
        </section>
    );
}

function Overview({ status, releases, packages }: { status: AgentStatus | null; releases: ReleasesResponse | null; packages: PackagesResponse | null }) {
    const health = status?.health;
    return <div className="uc-overview-layout"><SignedUpdatePanel status={status} releases={releases} />
        <section className="uc-panel"><div className="uc-panel-heading"><div><span className="uc-panel-kicker">SİSTEM SAĞLIĞI</span><h3>Update Agent bileşenleri</h3></div></div>{status ? <div className="uc-health-list"><HealthRow label="API / Agent" ok={Boolean(status.healthy ?? health?.healthy)} /><HealthRow label="PhotoOS Server HTTP" ok={Boolean(health?.server_http)} /><HealthRow label="Web arayüzü" ok={Boolean(health?.web_http)} /><HealthRow label="Veritabanı" ok={Boolean(health?.database_exists)} /><HealthRow label="Depolama" ok={Boolean(health?.storage_exists)} /></div> : <EmptyState title="Update Agent erişilemiyor" text="Update Agent bağlantısı sağlanamadığı için bileşen durumu gösterilemiyor." />}</section>
        <section className="uc-panel"><div className="uc-panel-heading"><div><span className="uc-panel-kicker">SÜRÜM ÖZETİ</span><h3>Release durumu</h3></div></div>{releases ? <div className="uc-version-flow"><div><span>Aktif sürüm</span><strong>{releases.active_release?.version || "Bilinmiyor"}</strong><small>{releases.active_release?.path || "Release yolu bildirilmedi"}</small></div><div><span>Kurulu release</span><strong>{releases.releases?.length ?? 0}</strong><small>{packages?.packages?.length ?? 0} paket sunucuda</small></div></div> : <EmptyState title="Release bilgisi alınamıyor" text="Update Agent release listesini döndürmedi." />}</section>
    </div>;
}

function HealthRow({ label, ok }: { label: string; ok: boolean }) {
    return <div className="uc-health-row"><span>{label}</span><span className={`uc-status-pill ${ok ? "ok" : "bad"}`}><i />{ok ? "Hazır" : "Kullanılamıyor"}</span></div>;
}

function PackagesPanel({ packages, selectedPackage, setSelectedPackage, action, uploadPackage, verifyPackage, installPackage }: { packages: PackagesResponse | null; selectedPackage: File | null; setSelectedPackage: (file: File | null) => void; action: string; uploadPackage: () => Promise<void>; verifyPackage: (filename: string) => Promise<void>; installPackage: (filename: string) => Promise<void> }) {
    const items = packages?.packages ?? [];
    return <div className="uc-packages-layout"><section className="uc-panel"><div className="uc-panel-heading"><div><span className="uc-panel-kicker">YENİ PAKET</span><h3>.popkg yükle</h3></div></div><label className={`uc-drop-zone ${selectedPackage ? "selected" : ""}`}><input type="file" accept=".popkg" onChange={(event) => setSelectedPackage(event.target.files?.[0] ?? null)} /><div className="uc-upload-icon">⇧</div><strong>{selectedPackage?.name || "PhotoOS paketini seçin"}</strong><span>{selectedPackage ? formatBytes(selectedPackage.size) : "Maksimum 512 MB · .popkg"}</span></label><button className="uc-primary-button" type="button" disabled={!selectedPackage || action === "upload"} onClick={() => void uploadPackage()}>{action === "upload" ? "Yükleniyor…" : "Paketi yükle"}</button></section><section className="uc-panel"><div className="uc-panel-heading"><div><span className="uc-panel-kicker">SUNUCUDAKİ PAKETLER</span><h3>Doğrula ve kur</h3></div><small>{items.length} paket</small></div>{!packages ? <EmptyState title="Paket listesi alınamıyor" text="Update Agent paket uç noktası erişilebilir değil." /> : items.length === 0 ? <EmptyState title="Paket bulunmuyor" text="Kurulum için bir .popkg dosyası yükleyin." /> : <div className="uc-package-list">{items.map((item) => <article className="uc-package-row" key={item.filename}><div className="uc-package-info"><strong>{packageVersion(item)}</strong><span>{item.filename}</span><small>{formatBytes(item.size_bytes)} · {item.verified || item.verification?.valid ? "Doğrulandı" : "Doğrulanmadı"}</small></div><div className="uc-package-actions"><button type="button" disabled={action === `verify:${item.filename}`} onClick={() => void verifyPackage(item.filename)}>{action === `verify:${item.filename}` ? "Doğrulanıyor…" : "Doğrula"}</button><button className="primary" type="button" disabled={!(item.verified || item.verification?.valid) || action === `install:${item.filename}`} onClick={() => void installPackage(item.filename)}>{action === `install:${item.filename}` ? "Kuruluyor…" : "Kur"}</button></div></article>)}</div>}</section></div>;
}

function ReleasesPanel({ releases, action, rollback }: { releases: ReleasesResponse | null; action: string; rollback: (version: string) => Promise<void> }) {
    const items = releases?.releases ?? [];
    return <section className="uc-panel"><div className="uc-panel-heading"><div><span className="uc-panel-kicker">RELEASE ENVANTERİ</span><h3>Kurulu PhotoOS sürümleri</h3></div><small>{items.length} release</small></div>{!releases ? <EmptyState title="Release listesi alınamıyor" text="Update Agent release uç noktası erişilebilir değil." /> : items.length === 0 ? <EmptyState title="Release bulunamadı" text="Update Agent kurulu release bildirmedi." /> : <div className="uc-release-list">{items.map((item) => <article className={`uc-release-row ${item.active ? "active" : ""}`} key={item.path}><div className="uc-release-info"><div><strong>PhotoOS {item.version}</strong><span className={`uc-status-pill ${item.active ? "ok" : "neutral"}`}><i />{item.active ? "Aktif" : "Kurulu"}</span></div><span>{item.path}</span><small>Server {item.server_exists ? "✓" : "—"} · Config {item.config_exists ? "✓" : "—"} · Migrations {item.migrations_exist ? "✓" : "—"} · Web {item.web_exists ? "✓" : "—"}</small></div><div className="uc-release-actions">{!item.active && <button type="button" disabled={action === `rollback:${item.version}`} onClick={() => void rollback(item.version)}>{action === `rollback:${item.version}` ? "İstek gönderiliyor…" : "Bu sürüme dön"}</button>}</div></article>)}</div>}</section>;
}

function HistoryPanel({ history }: { history: HistoryResponse | null }) {
    const items = history?.history ?? [];
    return <section className="uc-panel"><div className="uc-panel-heading"><div><span className="uc-panel-kicker">İŞLEM GEÇMİŞİ</span><h3>Update Agent kayıtları</h3></div><small>{formatDate(history?.timestamp)}</small></div>{!history ? <EmptyState title="Geçmiş alınamıyor" text="Update Agent geçmiş uç noktası erişilebilir değil." /> : items.length === 0 ? <EmptyState title="Henüz kayıt yok" text="Doğrulama, kurulum ve geri dönüş işlemleri burada görünür." /> : <div className="uc-history-list">{items.slice(0, 30).map((entry) => <details className="uc-history-row" key={entry.filename}><summary><strong>{entry.filename}</strong><span>Detayları aç</span></summary><pre>{JSON.stringify(entry.payload, null, 2)}</pre></details>)}</div>}</section>;
}

function EmptyState({ title, text }: { title: string; text: string }) {
    return <div className="uc-empty-state"><div>◇</div><strong>{title}</strong><p>{text}</p></div>;
}
