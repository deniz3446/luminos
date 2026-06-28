import { useEffect, useState } from "react";
import { createUser, getUsers } from "../api/users";

type User = {
    id: number;
    username: string;
    created_at: string;
};

function isValidEmail(email: string) {
    return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);
}

export default function UsersPage() {
    const [users, setUsers] = useState<User[]>([]);
    const [username, setUsername] = useState("");
    const [email, setEmail] = useState("");
    const [password, setPassword] = useState("");
    const [message, setMessage] = useState("");

    const formValid =
        username.trim().length >= 3 &&
        isValidEmail(email) &&
        password.length >= 8;

    async function loadUsers() {
        const data = await getUsers();
        setUsers(data);
    }

    useEffect(() => {
        loadUsers();
    }, []);

    async function handleCreateUser() {
        setMessage("");

        if (username.trim().length < 3) {
            setMessage("Kullanıcı adı en az 3 karakter olmalı.");
            return;
        }

        if (!isValidEmail(email)) {
            setMessage("Geçerli bir e-posta adresi gir.");
            return;
        }

        if (password.length < 8) {
            setMessage("Şifre en az 8 karakter olmalı.");
            return;
        }

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
                        <p>Geçerli e-posta ve en az 8 karakter şifre zorunlu.</p>
                    </div>
                </div>

                <div className="form-grid">
                    <input
                        placeholder="Kullanıcı adı"
                        value={username}
                        onChange={(e) => setUsername(e.target.value)}
                    />

                    <input
                        placeholder="E-posta"
                        value={email}
                        onChange={(e) => setEmail(e.target.value)}
                    />

                    <input
                        placeholder="Şifre (en az 8 karakter)"
                        type="password"
                        value={password}
                        onChange={(e) => setPassword(e.target.value)}
                    />

                    <button
                        className="primary-btn"
                        onClick={handleCreateUser}
                        disabled={!formValid}
                        style={{
                            opacity: formValid ? 1 : 0.45,
                            cursor: formValid ? "pointer" : "not-allowed",
                        }}
                    >
                        Kullanıcı Oluştur
                    </button>
                </div>

                {message && (
                    <p style={{ padding: "0 28px 24px", color: "#ef4444" }}>
                        {message}
                    </p>
                )}
            </section>

            <section className="panel">
                <div className="panel-header">
                    <div>
                        <h2>Kullanıcı Listesi</h2>
                        <p>Sistemde kayıtlı kullanıcılar</p>
                    </div>
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
