import { apiJson } from "./client";

export type PhotoListItem = {
    id?: number;
    filename: string;
    original_name?: string | null;
    mime_type?: string | null;
    size_bytes?: number | null;
    uploaded_at?: string | null;
    taken_at?: string | null;
    source_type?: string | null;
};

export async function listPhotos(limit = 100, offset = 0): Promise<PhotoListItem[]> {
    const payload = await apiJson<PhotoListItem[] | { photos?: PhotoListItem[] }>(`/api/v1/photos?limit=${limit}&offset=${offset}`);
    return Array.isArray(payload) ? payload : payload.photos ?? [];
}

export async function photoCount(): Promise<number> {
    const payload = await apiJson<number | { count?: number; total?: number }>("/api/v1/photos/count");
    if (typeof payload === "number") return payload;
    return payload.count ?? payload.total ?? 0;
}
