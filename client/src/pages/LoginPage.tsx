import { useState } from "react";

import { API_URL } from "../api/client";
import "./LoginPage.css";

export default function LoginPage() {
    const [email, setEmail] = useState("");
    const [password, setPassword] = useState("");
    const [message, setMessage] = useState("");
    const [visible, setVisible] = useState(false);
    const [submitting, setSubmitting] = useState(false);

    async function handleLogin(e: React.FormEvent) {
        e.preventDefault();
        if (submitting) return;
        setMessage("Giriş deneniyor...");
        setSubmitting(true);

        try {
            const response = await fetch(`${API_URL}/api/v1/login`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ email, password }),
            });
            const data = await response.json();
            if (data.success) {
                localStorage.setItem("token", data.data.token);
                window.location.href = "/dashboard";
            } else {
                setMessage(data.message || "E-posta veya şifre hatalı.");
            }
        } catch (error) {
            console.error(error);
            setMessage("Sunucuya bağlanılamadı.");
        } finally {
            setSubmitting(false);
        }
    }

    return (
        <main className="login-page">
            <div className="login-ambient login-ambient-a" aria-hidden="true" />
            <div className="login-ambient login-ambient-b" aria-hidden="true" />
            <div className="login-grid" aria-hidden="true" />

            <section className="login-shell">
                <div className="login-column">
                    <header className="login-brand">
                        <div className="login-logo" aria-hidden="true"><span>◇</span></div>
                        <div>
                            <strong>Photo<span>OS</span></strong>
                            <small>Private photo cloud</small>
                        </div>
                    </header>
                    <p className="login-tagline">Fotoğraflarınız. Verileriniz. Sizin bulutunuz.</p>

                    <form className="login-card" onSubmit={handleLogin}>
                        <span className="eyebrow"><span className="login-lock" aria-hidden="true">▣</span>GÜVENLİ ERİŞİM</span>
                        <h1>Hesabınıza Giriş Yapın</h1>
                        <p className="login-card-copy">Anılarınız her zaman yanınızda.</p>

                        <label htmlFor="login-email">E-posta</label>
                        <div className="login-input-wrap">
                            <span aria-hidden="true">✉</span>
                            <input id="login-email" type="email" autoComplete="username" placeholder="ornek@email.com" value={email} onChange={(event) => setEmail(event.target.value)} required />
                        </div>

                        <label htmlFor="login-password">Parola</label>
                        <div className="password-field login-input-wrap">
                            <span aria-hidden="true">▢</span>
                            <input id="login-password" type={visible ? "text" : "password"} autoComplete="current-password" placeholder="Parolanızı girin" value={password} onChange={(event) => setPassword(event.target.value)} required />
                            <button type="button" className="password-toggle" aria-label={visible ? "Parolayı gizle" : "Parolayı göster"} onClick={() => setVisible((current) => !current)}>{visible ? "◉" : "◌"}</button>
                        </div>

                        <button className="login-submit" type="submit" disabled={submitting}>
                            <span>{submitting ? "Giriş yapılıyor..." : "PhotoOS’a Giriş"}</span><b aria-hidden="true">→</b>
                        </button>
                        <output className="login-status" role="status" aria-live="polite">{message || ""}</output>

                        <div className="login-features" aria-label="PhotoOS güvenlik özellikleri">
                            <span><i aria-hidden="true">◇</i><b>Güvenle saklayın</b></span>
                            <span><i aria-hidden="true">▯</i><b>Her cihazdan erişin</b></span>
                            <span><i aria-hidden="true">≡</i><b>Tam kontrol sizde</b></span>
                        </div>
                    </form>
                </div>

                <div className="login-visual">
                    <div className="login-hero-copy">
                        <span className="eyebrow">YOUR PRIVATE CLOUD</span>
                        <h2>Anılarınızın<br /><em>güvenli merkezi.</em></h2>
                        <p>Tek bir yerde. Sizin donanımınızda. Sizin kurallarınızla.</p>
                    </div>

                    <div className="cloud-diagram" aria-label="PhotoOS kişisel bulut görselleştirmesi">
                        <div className="login-world" aria-hidden="true">
                            <div className="world-glow" />
                            <div className="world-sphere">
                                <span className="world-continent world-continent-a" />
                                <span className="world-continent world-continent-b" />
                                <span className="world-continent world-continent-c" />
                                <span className="world-latitude world-latitude-a" />
                                <span className="world-latitude world-latitude-b" />
                                <span className="world-longitude world-longitude-a" />
                                <span className="world-longitude world-longitude-b" />
                            </div>
                            <span className="world-orbit world-orbit-a" />
                            <span className="world-orbit world-orbit-b" />
                            <span className="world-orbit world-orbit-c" />
                        </div>

                        <div className="login-data-card login-data-card-a"><span>▧</span><div><b>FOTOĞRAFLAR</b><small>Anlar daima sizinle</small></div></div>
                        <div className="login-data-card login-data-card-b"><span>▷</span><div><b>VİDEOLAR</b><small>Yaşam hikayeniz</small></div></div>
                        <div className="login-data-card login-data-card-c"><span>▤</span><div><b>BELGELER</b><small>Önemli dosyalarınız</small></div></div>
                        <div className="login-data-card login-data-card-d"><span>☁</span><div><b>YEDEKLEME</b><small>Her zaman güvende</small></div></div>

                        <aside className="privacy-card">
                            <div className="privacy-shield" aria-hidden="true">◇</div>
                            <div><strong>VERİLERİNİZ</strong><b>SİZİN KONTROLÜNÜZDE</b></div>
                            <ul>
                                <li>Kendi sunucunuzda</li><li>Tam gizlilik</li><li>Güvenli erişim</li><li>Depolama sizin kontrolünüzde</li>
                            </ul>
                        </aside>
                    </div>

                    <div className="login-pillars" aria-hidden="true"><span>DAHA FAZLA AN</span><i>×</i><span>DAHA GÜVENLİ</span><i>×</i><span>DAHA ÖZGÜR</span></div>
                </div>
            </section>

            <footer className="login-footer">
                <span>Kendi sunucunuzda. Kendi kurallarınızla.</span><span>PhotoOS · Private Photo Cloud</span><span>Built for Your Memories</span>
            </footer>
        </main>
    );
}
