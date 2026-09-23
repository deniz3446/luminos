import { useEffect, useMemo, useState } from "react";

type Photo = {
    id: number;
    filename: string;
    original_name: string;
    url: string;
    size_bytes: number;
    mime_type: string;
    uploaded_at: string;
    taken_at: string | null;
    user_id: number;
    owner_username?: string;
};

type User = {
    id: number;
    username: string;
};

const API_URL = "http://localhost:8080";

const MONTHS = [
    "Ocak",
    "Şubat",
    "Mart",
    "Nisan",
    "Mayıs",
    "Haziran",
    "Temmuz",
    "Ağustos",
    "Eylül",
    "Ekim",
    "Kasım",
    "Aralık",
];

function parseDate(raw?: string | null): Date | null {
    if (!raw) return null;

    const fixed = raw.replace(" ", "T");
    const date = new Date(fixed);

    if (Number.isNaN(date.getTime())) return null;

    return date;
}

function getPhotoDate(photo: Photo): Date | null {
    return parseDate(photo.taken_at) || parseDate(photo.uploaded_at);
}

function getYear(photo: Photo): number | null {
    const date = getPhotoDate(photo);
    return date ? date.getFullYear() : null;
}

function getMonth(photo: Photo): number | null {
    const date = getPhotoDate(photo);
    return date ? date.getMonth() : null;
}

function getDay(photo: Photo): number | null {
    const date = getPhotoDate(photo);
    return date ? date.getDate() : null;
}

export default function PhotosPage() {
    const [photos, setPhotos] = useState<Photo[]>([]);
    const [users, setUsers] = useState<User[]>([]);

    const [selectedUserId, setSelectedUserId] = useState<number | "all">("all");
    const [selectedYear, setSelectedYear] = useState<number | null>(null);
    const [selectedMonth, setSelectedMonth] = useState<number | null>(null);
    const [selectedDay, setSelectedDay] = useState<number | null>(null);

    const [selectionMode, setSelectionMode] = useState(false);
    const [selectedPhotoIds, setSelectedPhotoIds] = useState<number[]>([]);

    useEffect(() => {
        loadPhotos();
        loadUsers();
    }, []);

    async function loadPhotos() {
        const res = await fetch(`${API_URL}/api/v1/photos?limit=5000&offset=0`);
        const data = await res.json();
        setPhotos(data);
    }

    async function loadUsers() {
        const res = await fetch(`${API_URL}/api/v1/users`);
        const data = await res.json();
        setUsers(data);
    }

    const filteredPhotos = useMemo(() => {
        if (selectedUserId === "all") return photos;
        return photos.filter((photo) => photo.user_id === selectedUserId);
    }, [photos, selectedUserId]);

    const years = useMemo(() => {
        const map = new Map<number, number>();

        for (const photo of filteredPhotos) {
            const year = getYear(photo);
            if (!year) continue;

            map.set(year, (map.get(year) || 0) + 1);
        }

        return Array.from(map.entries())
            .map(([year, count]) => ({ year, count }))
            .sort((a, b) => b.year - a.year);
    }, [filteredPhotos]);

    useEffect(() => {
        if (years.length > 0 && selectedYear === null) {
            setSelectedYear(years[0].year);
        }
    }, [years, selectedYear]);

    const months = useMemo(() => {
        if (selectedYear === null) return [];

        const map = new Map<number, number>();

        for (const photo of filteredPhotos) {
            if (getYear(photo) !== selectedYear) continue;

            const month = getMonth(photo);
            if (month === null) continue;

            map.set(month, (map.get(month) || 0) + 1);
        }

        return Array.from(map.entries())
            .map(([month, count]) => ({ month, count }))
            .sort((a, b) => b.month - a.month);
    }, [filteredPhotos, selectedYear]);

    useEffect(() => {
        if (months.length > 0 && selectedMonth === null) {
            setSelectedMonth(months[0].month);
        }
    }, [months, selectedMonth]);

    const days = useMemo(() => {
        if (selectedYear === null || selectedMonth === null) return [];

        const map = new Map<number, number>();

        for (const photo of filteredPhotos) {
            if (getYear(photo) !== selectedYear) continue;
            if (getMonth(photo) !== selectedMonth) continue;

            const day = getDay(photo);
            if (day === null) continue;

            map.set(day, (map.get(day) || 0) + 1);
        }

        return Array.from(map.entries())
            .map(([day, count]) => ({ day, count }))
            .sort((a, b) => b.day - a.day);
    }, [filteredPhotos, selectedYear, selectedMonth]);

    useEffect(() => {
        if (days.length > 0 && selectedDay === null) {
            setSelectedDay(days[0].day);
        }
    }, [days, selectedDay]);

    const visiblePhotos = useMemo(() => {
        return filteredPhotos.filter((photo) => {
            if (selectedYear !== null && getYear(photo) !== selectedYear) return false;
            if (selectedMonth !== null && getMonth(photo) !== selectedMonth) return false;
            if (selectedDay !== null && getDay(photo) !== selectedDay) return false;

            return true;
        });
    }, [filteredPhotos, selectedYear, selectedMonth, selectedDay]);

    function changeUser(value: string) {
        setSelectedUserId(value === "all" ? "all" : Number(value));
        setSelectedYear(null);
        setSelectedMonth(null);
        setSelectedDay(null);
        setSelectedPhotoIds([]);
    }

    function changeYear(year: number) {
        setSelectedYear(year);
        setSelectedMonth(null);
        setSelectedDay(null);
        setSelectedPhotoIds([]);
    }

    function changeMonth(month: number) {
        setSelectedMonth(month);
        setSelectedDay(null);
        setSelectedPhotoIds([]);
    }

    function changeDay(day: number) {
        setSelectedDay(day);
        setSelectedPhotoIds([]);
    }

    function togglePhoto(id: number) {
        if (!selectionMode) return;

        setSelectedPhotoIds((prev) =>
            prev.includes(id)
                ? prev.filter((x) => x !== id)
                : [...prev, id],
        );
    }

    return (
        <div className="photos-page">
            <div className="photos-header">
                <div>
                    <h1>Fotoğraflar</h1>
                </div>

                <div className="photos-actions">
                    <button
                        onClick={() => {
                            setSelectionMode(!selectionMode);
                            setSelectedPhotoIds([]);
                        }}
                    >
                        {selectionMode ? "Seçimi Kapat" : "Seçim Modu"}
                    </button>

                    <select
                        value={selectedUserId}
                        onChange={(e) => changeUser(e.target.value)}
                    >
                        <option value="all">Tüm kullanıcılar</option>
                        {users.map((user) => (
                            <option key={user.id} value={user.id}>
                                {user.username}
                            </option>
                        ))}
                    </select>
                </div>
            </div>

            <div className="year-list">
                {years.map((item) => (
                    <button
                        key={item.year}
                        className={selectedYear === item.year ? "active" : ""}
                        onClick={() => changeYear(item.year)}
                    >
                        <div className="year-title">🗓️ {item.year}</div>
                        <div className="year-count">{item.count} medya</div>
                    </button>
                ))}
            </div>

            <div className="month-list">
                {months.map((item) => (
                    <button
                        key={item.month}
                        className={selectedMonth === item.month ? "active" : ""}
                        onClick={() => changeMonth(item.month)}
                    >
                        <div>{MONTHS[item.month]}</div>
                        <small>{item.count} medya</small>
                    </button>
                ))}
            </div>

            <div className="day-list">
                {days.map((item) => (
                    <button
                        key={item.day}
                        className={selectedDay === item.day ? "active" : ""}
                        onClick={() => changeDay(item.day)}
                    >
                        <strong>{String(item.day).padStart(2, "0")}</strong>
                        <small>{item.count}</small>
                    </button>
                ))}
            </div>

            <div className="photo-grid">
                {visiblePhotos.map((photo) => {
                    const selected = selectedPhotoIds.includes(photo.id);

                    return (
                        <div
                            key={photo.id}
                            className={`photo-card ${selected ? "selected" : ""}`}
                            onClick={() => togglePhoto(photo.id)}
                        >
                            {selectionMode && (
                                <div className="photo-check">
                                    {selected ? "✓" : ""}
                                </div>
                            )}

                            <img
                                src={`${API_URL}${photo.url}`}
                                alt={photo.original_name}
                                loading="lazy"
                            />

                            <div className="photo-info">
                                <span>{photo.original_name}</span>
                                <small>{photo.owner_username || "Bilinmeyen"}</small>
                            </div>
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
