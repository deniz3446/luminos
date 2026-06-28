import { Link, Outlet } from "react-router-dom";

export default function DashboardLayout() {
    function logout() {
        localStorage.removeItem("token");
        window.location.href = "/";
    }

    return (
        <div className="app-shell">
            <aside className="sidebar">
                <div className="brand">
                    <div className="brand-icon">📷</div>
                    <div>
                        <h2>PhotoOS</h2>
                        <span>LuminOS Server</span>
                    </div>
                </div>

                <nav className="nav">
                    <Link to="/dashboard">Dashboard</Link>
                    <Link to="/users">Kullanıcılar</Link>
                    <Link to="/albums">Albümler</Link>
                    <Link to="/photos">Fotoğraflar</Link>
                </nav>

                <button className="logout-btn" onClick={logout}>
                    Çıkış Yap
                </button>
            </aside>

            <main className="main">
                <header className="topbar">
                    <div>
                        <h1>PhotoOS Panel</h1>
                        <p>Kişisel fotoğraf sunucun</p>
                    </div>

                    <div className="user-pill">
                        deniz
                    </div>
                </header>

                <section className="content">
                    <Outlet />
                </section>
            </main>
        </div>
    );
}
