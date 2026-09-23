import { useCallback, useEffect, useState } from "react";
import {
    createUser,
    getUsers,
    type UserSummary,
} from "../api/users";

function isValidEmail(email: string) {
    return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);
}

function formatUserDate(raw: string) {
    if (!raw) return "-";
    const fixed = raw.includes("T") ? raw : raw.replace(" ", "T");
    const date = new Date(fixed);
    if (Number.isNaN(date.getTime())) return raw;
    return date.toLocaleString("tr-TR", {
        day: "2-digit",
        month: "2-digit",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
    });
}

export default function UsersPage() {
    const [users, setUsers] = useState<UserSummary[]>([]);
    const [username, setUsername] = useState("");
    const [email, setEmail] = useState("");
    const [password, setPassword] = useState("");
    const [message, setMessage] = useState("");
    const [loading, setLoading] = useState(true);

    const formValid =
        username.trim().length >= 3 &&
        isValidEmail(email) &&
        password.length >= 8;

    const loadUsers = useCallback(async () => {
        try {
            setLoading(true);
            setUsers(await getUsers());
        } catch (error) {
            console.error("Kullanıcılar alınamadı:", error);
            setMessage("Kullanıcı listesi alınamadı.");
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => {
        const timer = window.setTimeout(() => void loadUsers(), 0);
        return () => window.clearTimeout(timer);
    }, [loadUsers]);

    async function handleCreateUser() {
        setMessage("");
        if (!formValid) {
            setMessage("Bilgileri kontrol et.");
            return;
        }

        try {
            const result = await createUser({
                username: username.trim(),
                email: email.trim(),
                password,
            });
            setMessage(result.message);
            if (result.success) {
                setUsername("");
                setEmail("");
                setPassword("");
                await loadUsers();
            }
        } catch (error) {
            setMessage(
                error instanceof Error ? error.message : "Kullanıcı oluşturulamadı.",
            );
        }
    }

    return (
        <>
            <div className="page-title">
                <h1>Kullanıcılar</h1>
                <p>PhotoOS kullanıcı yönetimi</p>
            </div>

            <section className="panel">
                <div className="panel-header">
                    <div>
                        <h2>Yeni Kullanıcı</h2>
                        <p>Yeni hesaplar standart kullanıcı rolüyle oluşturulur.</p>
                    </div>
                </div>

                <div className="form-grid">
                    <input
                        placeholder="Kullanıcı adı"
                        value={username}
                        onChange={(event) => setUsername(event.target.value)}
                    />
                    <input
                        placeholder="E-posta"
                        value={email}
                        onChange={(event) => setEmail(event.target.value)}
                    />
                    <input
                        placeholder="Şifre (en az 8 karakter)"
                        type="password"
                        value={password}
                        onChange={(event) => setPassword(event.target.value)}
                    />
                    <button
                        className="primary-btn"
                        onClick={() => void handleCreateUser()}
                        disabled={!formValid}
                    >
                        Kullanıcı Oluştur
                    </button>
                </div>

                {message && (
                    <p style={{ padding: "0 28px 24px" }}>{message}</p>
                )}
            </section>

            <section className="panel">
                <div className="panel-header">
                    <div>
                        <h2>Kullanıcı Listesi</h2>
                        <p>Sistemde kayıtlı kullanıcılar</p>
                    </div>
                </div>

                {loading ? (
                    <p style={{ padding: "0 28px 24px" }}>Yükleniyor...</p>
                ) : (
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
                                    <td>{formatUserDate(user.created_at)}</td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                )}
            </section>
        </>
    );
}
