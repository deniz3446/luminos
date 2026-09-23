import { api, apiJson } from "./client";

export type Album = {
    id: number;
    user_id: number;
    title: string;
    description?: string | null;
    created_at: string;
    photo_count: number;
    cover_url?: string | null;
};

export type AlbumPhoto = {
    id: number;
    filename: string;
    original_name: string;
    url: string;
    size_bytes: number;
    mime_type: string;
    uploaded_at: string;
    taken_at?: string | null;
    user_id: number;
    owner_username: string;
};

export type AlbumDetail = {
    album: Album;
    photos: AlbumPhoto[];
};

export type CreateAlbumInput = {
    title: string;
    description?: string;
};

async function requireOk(response: Response): Promise<void> {
    if (response.ok) return;
    throw new Error(`HTTP ${response.status}`);
}

export async function getAlbums(): Promise<Album[]> {
    return apiJson<Album[]>("/api/v1/albums");
}

export const listAlbums = getAlbums;

export async function createAlbum(data: CreateAlbumInput): Promise<Album> {
    return apiJson<Album>("/api/v1/albums", {
        method: "POST",
        body: JSON.stringify(data),
    });
}

export async function getAlbum(id: number | string): Promise<AlbumDetail> {
    return apiJson<AlbumDetail>(
        `/api/v1/albums/${encodeURIComponent(String(id))}`,
    );
}

export async function deleteAlbum(id: number | string): Promise<void> {
    const response = await api(
        `/api/v1/albums/${encodeURIComponent(String(id))}`,
        { method: "DELETE" },
    );
    await requireOk(response);
}

export async function addPhotosToAlbum(
    albumId: number | string,
    photoIds: number[],
): Promise<void> {
    const response = await api(
        `/api/v1/albums/${encodeURIComponent(String(albumId))}/photos`,
        {
            method: "POST",
            body: JSON.stringify({ photo_ids: photoIds }),
        },
    );
    await requireOk(response);
}

export async function removePhotoFromAlbum(
    albumId: number | string,
    photoId: number | string,
): Promise<void> {
    const response = await api(
        `/api/v1/albums/${encodeURIComponent(String(albumId))}/photos/${encodeURIComponent(String(photoId))}`,
        { method: "DELETE" },
    );
    await requireOk(response);
}
