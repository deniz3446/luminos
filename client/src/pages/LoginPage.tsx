import { useState } from "react";

export default function LoginPage() {
    const [email, setEmail] = useState("deniz@test.com");
    const [password, setPassword] = useState("123456");
    const [message, setMessage] = useState("");

    async function handleLogin() {
        setMessage("Giriş deneniyor...");

        try {
            const response = await fetch(
                "http://192.168.1.41:8080/api/v1/login",
                {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json",
                    },
                    body: JSON.stringify({
                        email,
                        password,
                    }),
                }
            );

            const data = await response.json();

            if (data.success) {
                localStorage.setItem("token", data.data.token);
                window.location.href = "/dashboard";
            } else {
                setMessage(data.message);
            }
        } catch (error) {
            console.error(error);
            setMessage("Sunucuya bağlanılamadı.");
        }
    }

    return (
        <div
            style={{
                display: "flex",
                justifyContent: "center",
                alignItems: "center",
                height: "100vh",
                fontFamily: "Arial",
                background: "#111827",
                color: "white",
            }}
        >
            <div>
                <h1>PhotoOS</h1>
                <p>Giriş Sayfası</p>

                <input
                    placeholder="E-posta"
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                />

                <br />
                <br />

                <input
                    placeholder="Şifre"
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                />

                <br />
                <br />

                <button onClick={handleLogin}>
                    Giriş Yap
                </button>

                <p>{message}</p>
            </div>
        </div>
    );
}
