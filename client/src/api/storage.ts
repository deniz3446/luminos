import { api } from "./client";

export type StorageInfo = {
    total_bytes: number;
    used_bytes: number;
    free_bytes: number;
};

export async function getStorageInfo(): Promise<StorageInfo> {
    const res = await api("/api/v1/storage");
    return res.json();
}
