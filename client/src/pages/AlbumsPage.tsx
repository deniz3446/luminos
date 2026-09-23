import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { createAlbum, getAlbums, type Album } from "../api/albums";

import { API_URL } from "../api/client";

type Photo = {
    id: number;
    filename: string;
    original_name: string;
    url: string;
    size_bytes: number;
    mime_type: string;
    uploaded_at: string;
    taken_at?: string | null;
    owner_username?: string;
};

function isVideo(photo: Photo) {
    return photo.mime_type.startsWith("video/");
}

function AlbumCard({
    title,
    subtitle,
    icon,
    cover,
    onClick,
}: {
    title: string;
    subtitle: string;
    icon: string;
    cover?: Photo;
    onClick: () => void;
}) {
    return (
        <button
            onClick={onClick}
            style={{
                textAlign: "left",
                border: "1px solid rgba(239,68,68,0.22)",
                borderRadius: 22,
                overflow: "hidden",
                background: "linear-gradient(180deg, #141414 0%, #070707 100%)",
                color: "#fff",
                padding: 0,
                cursor: "pointer",
                boxShadow: "0 16px 35px rgba(0,0,0,0.32)",
            }}
        >
            <div
                style={{
                    height: 180,
                    background: "#090909",
                    position: "relative",
                    overflow: "hidden",
                }}
            >
                {cover ? (
                    <img
                        src={`${API_URL}${cover.url}`}
                        alt={title}
                        style={{
                            width: "100%",
                            height: "100%",
                            objectFit: "cover",
                            display: "block",
                            opacity: 0.86,
                        }}
                    />
                ) : (
                    <div
                        style={{
                            height: "100%",
                            display: "flex",
                            alignItems: "center",
                            justifyContent: "center",
                            fontSize: 54,
                        }}
                    >
                        {icon}
                    </div>
                )}

                <div
                    style={{
                        position: "absolute",
                        inset: 0,
                        background:
                            "linear-gradient(to top, rgba(0,0,0,0.86), rgba(0,0,0,0.12), transparent)",
                    }}
                />

                <div
                    style={{
                        position: "absolute",
                        left: 16,
                        bottom: 14,
                        fontSize: 36,
                    }}
                >
                    {icon}
                </div>
            </div>

            <div style={{ padding: 18 }}>
                <h2 style={{ margin: 0, fontSize: 21 }}>{title}</h2>
                <p style={{ margin: "8px 0 0", color: "#fca5a5", fontWeight: 700 }}>
                    {subtitle}
                </p>
            </div>
        </button>
    );
}

function UserAlbumCard({
    album,
    onClick,
}: {
    album: Album;
    onClick: () => void;
}) {
    return (
        <button
            onClick={onClick}
            style={{
                textAlign: "left",
                border: "1px solid rgba(239,68,68,0.22)",
                borderRadius: 22,
                overflow: "hidden",
                background: "linear-gradient(180deg, #141414 0%, #070707 100%)",
                color: "#fff",
                padding: 0,
                cursor: "pointer",
                boxShadow: "0 16px 35px rgba(0,0,0,0.32)",
            }}
        >
            <div
                style={{
                    height: 180,
                    background: "#090909",
                    position: "relative",
                    overflow: "hidden",
                }}
            >
                {album.cover_url ? (
                    <img
                        src={`${API_URL}${album.cover_url}`}
                        alt={album.title}
                        style={{
                            width: "100%",
                            height: "100%",
                            objectFit: "cover",
                            display: "block",
                            opacity: 0.86,
                        }}
                    />
                ) : (
                    <div
                        style={{
                            height: "100%",
                            display: "flex",
                            alignItems: "center",
                            justifyContent: "center",
                            fontSize: 54,
                        }}
                    >
                        📚
                    </div>
                )}

                <div
                    style={{
                        position: "absolute",
                        inset: 0,
                        background:
                            "linear-gradient(to top, rgba(0,0,0,0.86), rgba(0,0,0,0.12), transparent)",
                    }}
                />
            </div>

            <div style={{ padding: 18 }}>
                <h2 style={{ margin: 0, fontSize: 21 }}>{album.title}</h2>
                <p style={{ margin: "8px 0 0", color: "#fca5a5", fontWeight: 700 }}>
                    {album.photo_count} medya
                </p>
                {album.description && (
                    <p style={{ margin: "10px 0 0", color: "#aaa", fontSize: 14 }}>
                        {album.description}
                    </p>
                )}
            </div>
        </button>
    );
}

export default function AlbumsPage() {
    const navigate = useNavigate();
    const [photos, setPhotos] = useState<Photo[]>([]);
    const [albums, setAlbums] = useState<Album[]>([]);
    const [showCreate, setShowCreate] = useState(false);
    const [newTitle, setNewTitle] = useState("");
    const [newDescription, setNewDescription] = useState("");
    const [creating, setCreating] = useState(false);

    async function loadAlbums() {
        try {
            const data = await getAlbums();
            setAlbums(Array.isArray(data) ? data : []);
        } catch (err) {
            console.error("Albümler alınamadı:", err);
            setAlbums([]);
        }
    }

    useEffect(() => {
        fetch(`${API_URL}/api/v1/photos`, {
            headers: {
                Authorization: `Bearer ${localStorage.getItem("token")}`,
            },
        })
            .then((res) => res.json())
            .then((data) => setPhotos(Array.isArray(data) ? data : []))
            .catch(() => setPhotos([]));

        loadAlbums();
    }, []);

    const videos = useMemo(() => photos.filter(isVideo), [photos]);
    const images = useMemo(() => photos.filter((p) => !isVideo(p)), [photos]);

    const thisYear = useMemo(() => {
        const year = new Date().getFullYear();
        return photos.filter((p) => {
            const d = new Date(p.taken_at || p.uploaded_at);
            return !Number.isNaN(d.getTime()) && d.getFullYear() === year;
        });
    }, [photos]);

    const thisMonth = useMemo(() => {
        const now = new Date();
        return photos.filter((p) => {
            const d = new Date(p.taken_at || p.uploaded_at);
            return (
                !Number.isNaN(d.getTime()) &&
                d.getFullYear() === now.getFullYear() &&
                d.getMonth() === now.getMonth()
            );
        });
    }, [photos]);

    async function handleCreateAlbum() {
        const title = newTitle.trim();
        if (!title) {
            alert("Albüm adı boş olamaz.");
            return;
        }

        try {
            setCreating(true);
            await createAlbum({
                title,
                description: newDescription.trim() || undefined,
            });

            setNewTitle("");
            setNewDescription("");
            setShowCreate(false);
            await loadAlbums();
        } catch (err) {
            console.error(err);
            alert("Albüm oluşturulamadı.");
        } finally {
            setCreating(false);
        }
    }

    return (
        <div>
            <div
                style={{
                    display: "flex",
                    justifyContent: "space-between",
                    alignItems: "center",
                    marginBottom: 24,
                    gap: 14,
                    flexWrap: "wrap",
                }}
            >
                <div>
                    <h1 style={{ margin: 0, fontSize: 32 }}>Albümler</h1>
                    <p style={{ margin: "8px 0 0", color: "#aaa" }}>
                        Fotoğraf ve video koleksiyonları
                    </p>
                </div>

                <button
                    onClick={() => setShowCreate((prev) => !prev)}
                    style={{
                        height: 44,
                        borderRadius: 14,
                        border: "1px solid #7f1d1d",
                        background: "#120406",
                        color: "#fff",
                        padding: "0 16px",
                        fontWeight: 800,
                        cursor: "pointer",
                    }}
                >
                    + Yeni Albüm
                </button>
            </div>

            {showCreate && (
                <div
                    style={{
                        marginBottom: 24,
                        padding: 18,
                        borderRadius: 18,
                        background: "linear-gradient(145deg, #120406, #080808)",
                        border: "1px solid rgba(239,68,68,0.22)",
                    }}
                >
                    <h3 style={{ marginTop: 0 }}>Yeni Albüm Oluştur</h3>

                    <div style={{ display: "grid", gap: 12 }}>
                        <input
                            value={newTitle}
                            onChange={(e) => setNewTitle(e.target.value)}
                            placeholder="Albüm adı"
                            style={{
                                height: 44,
                                borderRadius: 12,
                                border: "1px solid #3f3f46",
                                background: "#111",
                                color: "#fff",
                                padding: "0 14px",
                            }}
                        />

                        <textarea
                            value={newDescription}
                            onChange={(e) => setNewDescription(e.target.value)}
                            placeholder="Açıklama (opsiyonel)"
                            rows={3}
                            style={{
                                borderRadius: 12,
                                border: "1px solid #3f3f46",
                                background: "#111",
                                color: "#fff",
                                padding: "12px 14px",
                                resize: "vertical",
                            }}
                        />

                        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
                            <button
                                onClick={handleCreateAlbum}
                                disabled={creating}
                                style={{
                                    height: 42,
                                    borderRadius: 12,
                                    border: "1px solid #7f1d1d",
                                    background: creating ? "#3f3f46" : "#991b1b",
                                    color: "#fff",
                                    padding: "0 16px",
                                    fontWeight: 800,
                                    cursor: creating ? "not-allowed" : "pointer",
                                }}
                            >
                                {creating ? "Oluşturuluyor..." : "Albümü Oluştur"}
                            </button>

                            <button
                                onClick={() => {
                                    setShowCreate(false);
                                    setNewTitle("");
                                    setNewDescription("");
                                }}
                                style={{
                                    height: 42,
                                    borderRadius: 12,
                                    border: "1px solid #3f3f46",
                                    background: "#111",
                                    color: "#fff",
                                    padding: "0 16px",
                                    fontWeight: 800,
                                    cursor: "pointer",
                                }}
                            >
                                Vazgeç
                            </button>
                        </div>
                    </div>
                </div>
            )}

            <section style={{ marginBottom: 28 }}>
                <h2 style={{ marginTop: 0, marginBottom: 14 }}>Akıllı Albümler</h2>

                <div
                    style={{
                        display: "grid",
                        gridTemplateColumns: "repeat(5, minmax(0, 1fr))",
                        gap: 16,
                    }}
                >
                    <AlbumCard
                        title="Tüm Medya"
                        subtitle={`${photos.length} medya`}
                        icon="🖼"
                        cover={photos[0]}
                        onClick={() => navigate("/photos")}
                    />

                    <AlbumCard
                        title="Fotoğraflar"
                        subtitle={`${images.length} fotoğraf`}
                        icon="📷"
                        cover={images[0]}
                        onClick={() => navigate("/photos")}
                    />

                    <AlbumCard
                        title="Videolar"
                        subtitle={`${videos.length} video`}
                        icon="🎬"
                        cover={videos[0]}
                        onClick={() => navigate("/photos")}
                    />

                    <AlbumCard
                        title="Bu Ay"
                        subtitle={`${thisMonth.length} medya`}
                        icon="📅"
                        cover={thisMonth[0]}
                        onClick={() => navigate("/photos")}
                    />

                    <AlbumCard
                        title="Bu Yıl"
                        subtitle={`${thisYear.length} medya`}
                        icon="🗓"
                        cover={thisYear[0]}
                        onClick={() => navigate("/photos")}
                    />
                </div>
            </section>

            <section>
                <div
                    style={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                        marginBottom: 14,
                        gap: 12,
                        flexWrap: "wrap",
                    }}
                >
                    <h2 style={{ margin: 0 }}>Gerçek Albümler</h2>
                    <div style={{ color: "#fca5a5", fontWeight: 700 }}>
                        {albums.length} albüm
                    </div>
                </div>

                {albums.length === 0 ? (
                    <div
                        style={{
                            padding: 22,
                            borderRadius: 18,
                            background: "#0b0b0b",
                            border: "1px solid rgba(239,68,68,0.16)",
                            color: "#aaa",
                        }}
                    >
                        Henüz oluşturulmuş albüm yok.
                    </div>
                ) : (
                    <div
                        style={{
                            display: "grid",
                            gridTemplateColumns: "repeat(4, minmax(0, 1fr))",
                            gap: 16,
                        }}
                    >
                        {albums.map((album) => (
                            <UserAlbumCard
                                key={album.id}
                                album={album}
                                onClick={() => navigate(`/albums/${album.id}`)}
                            />
                        ))}
                    </div>
                )}
            </section>
        </div>
    );
}
