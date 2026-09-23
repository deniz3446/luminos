import { Link } from "react-router-dom";

export default function NotFoundPage() {
  return (
    <main
      aria-labelledby="not-found-title"
      style={{
        minHeight: "100vh",
        display: "grid",
        placeItems: "center",
        padding: "24px",
        background: "var(--app-bg, #f5f6f8)",
      }}
    >
      <section
        style={{
          width: "min(520px, 100%)",
          padding: "32px",
          borderRadius: "16px",
          background: "var(--panel-bg, #fff)",
          boxShadow: "0 12px 40px rgba(0,0,0,.08)",
          textAlign: "center",
        }}
      >
        <div
          aria-hidden="true"
          style={{
            fontSize: "56px",
            fontWeight: 700,
            lineHeight: 1,
            marginBottom: "16px",
          }}
        >
          404
        </div>

        <h1 id="not-found-title">Sayfa bulunamadı</h1>

        <p>
          İstediğiniz sayfa mevcut değil veya PhotoOS'tan kaldırılmış olabilir.
        </p>

        <Link to="/dashboard">
          Ana panele dön
        </Link>
      </section>
    </main>
  );
}
