import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";

type User = {
    id: number;
    username: string;
    created_at: string;
};

export default function DashboardPage() {
    const navigate = useNavigate();

    const [users, setUsers] = useState<User[]>([]);

    useEffect(() => {
        fetch("http://192.168.1.41:8080/api/v1/users")
            .then((res) => res.json())
            .then((data) => setUsers(data))
            .catch(() => setUsers([]));
    }, []);

    return (
        <>
            <div className="page-title">
                <h1>Dashboard</h1>
                <p>PhotoOS Yönetim Paneli</p>
            </div>

            <div className="stats-grid">
                <StatCard
                    title="Kullanıcılar"
                    value={users.length}
                    icon="👥"
                />

                <StatCard
                    title="Fotoğraflar"
                    value={0}
                    icon="📷"
                />

                <StatCard
                    title="Videolar"
                    value={0}
                    icon="🎥"
                />

                <StatCard
                    title="Depolama"
                    value={0}
                    icon="💾"
                />
            </div>

            <section className="panel">
                <div className="panel-header">
                    <div>
                        <h2>Kullanıcılar</h2>
                        <p>Sistemde kayıtlı kullanıcılar</p>
                    </div>

                    <button
                        className="primary-btn"
                        onClick={() => navigate("/users")}
                    >
                        Yeni Kullanıcı
                    </button>
                </div>

                <table className="data-table">
                    <thead>
                        <tr>
                            <th>ID</th>
                            <th>Kullanıcı</th>
                            <th>Oluşturulma</th>
                        </tr>
                    </thead>

                    <tbody>
                        {users.map((user) => (
                            <tr key={user.id}>
                                <td>{user.id}</td>
                                <td>{user.username}</td>
                                <td>{user.created_at}</td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            </section>
        </>
    );
}

function StatCard({
    title,
    value,
    icon,
}: {
    title: string;
    value: number;
    icon: string;
}) {
    return (
        <div className="stat-card">
            <div className="stat-icon">
                {icon}
            </div>

            <div className="stat-content">
                <h3>{title}</h3>
                <strong>{value}</strong>
            </div>
        </div>
    );
}
