export type PhotoLike = {
    filename: string;
    original_name?: string | null;
    owner_username?: string | null;
    source_type?: string | null;
    device_name?: string | null;
    mime_type?: string | null;
    taken_at?: string | null;
    uploaded_at?: string | null;
};

export type PhotoFilters = {
    owner?: string;
    source?: string;
    media?: string;
    search?: string;
};

export const SOURCE_OPTIONS = [
    { value: "all", label: "Tümü", icon: "▦" },
    { value: "camera", label: "Kamera", icon: "▣" },
    { value: "whatsapp_received", label: "WhatsApp Gelen", icon: "◉" },
    { value: "whatsapp_sent", label: "WhatsApp Gönderilen", icon: "↗" },
    { value: "screenshot", label: "Ekran Görüntüleri", icon: "▤" },
    { value: "download", label: "İndirilenler", icon: "↓" },
    { value: "telegram", label: "Telegram", icon: "➤" },
    { value: "pc_backup", label: "PC Backup", icon: "▰" },
    { value: "other", label: "Diğer", icon: "◇" },
] as const;

const knownSources = new Set(SOURCE_OPTIONS.slice(1).map((item) => item.value));

export function normalizeSourceType(value?: string | null): string {
    return value && knownSources.has(value as never) ? value : "other";
}

export function parsePhotoDate(raw?: string | null): Date | null {
    if (!raw) return null;
    const date = new Date(raw.replace(" ", "T"));
    return Number.isNaN(date.getTime()) ? null : date;
}

export function getPhotoDate(photo: PhotoLike): Date | null {
    return parsePhotoDate(photo.taken_at) || parsePhotoDate(photo.uploaded_at);
}

function normalizeText(value?: string | null): string {
    return (value ?? "").toLocaleLowerCase("tr-TR");
}

export function matchesPhotoSearch(photo: PhotoLike, query: string): boolean {
    const needle = normalizeText(query).trim();
    if (!needle) return true;
    const source = SOURCE_OPTIONS.find((item) => item.value === normalizeSourceType(photo.source_type));
    return [photo.filename, photo.original_name, photo.owner_username, photo.device_name, source?.label]
        .some((value) => normalizeText(value).includes(needle));
}

export function filterPhotoCollection<T extends PhotoLike>(photos: T[], filters: PhotoFilters): T[] {
    return photos.filter((photo) => {
        if (filters.owner && filters.owner !== "all" && (photo.owner_username || "Bilinmeyen") !== filters.owner) return false;
        if (filters.source && filters.source !== "all" && normalizeSourceType(photo.source_type) !== filters.source) return false;
        if (filters.media === "photo" && photo.mime_type?.startsWith("video/")) return false;
        if (filters.media === "video" && !photo.mime_type?.startsWith("video/")) return false;
        return matchesPhotoSearch(photo, filters.search ?? "");
    });
}
