const configuredBase = (import.meta.env.VITE_API_URL as string | undefined)?.trim();

export const API_URL = (configuredBase || (typeof window !== "undefined" ? window.location.origin : ""))
    .replace(/\/$/, "");

const TOKEN_KEYS = ["token", "photoos_token", "authToken"] as const;

export function getStoredToken(): string | null {
    if (typeof window === "undefined") return null;
    for (const key of TOKEN_KEYS) {
        const value = window.localStorage.getItem(key);
        if (value) return value;
    }
    return null;
}

export function clearStoredToken(): void {
    if (typeof window === "undefined") return;
    for (const key of TOKEN_KEYS) window.localStorage.removeItem(key);
}

export async function api(path: string, init: RequestInit = {}): Promise<Response> {
    const token = getStoredToken();
    const headers = new Headers(init.headers ?? {});
    if (token && !headers.has("Authorization")) headers.set("Authorization", `Bearer ${token}`);
    if (init.body && !(init.body instanceof FormData) && !headers.has("Content-Type")) {
        headers.set("Content-Type", "application/json");
    }
    if (/^https?:\/\//i.test(path)) {
        throw new Error("api() absolute URL kabul etmez.");
    }
    const url = `${API_URL}${path.startsWith("/") ? path : `/${path}`}`;
    return fetch(url, { ...init, headers });
}

export async function apiJson<T>(path: string, init: RequestInit = {}): Promise<T> {
    const response = await api(path, init);
    const text = await response.text();
    let payload: unknown = null;
    if (text) {
        try { payload = JSON.parse(text); } catch { payload = { message: text }; }
    }
    if (!response.ok) {
        const record = payload && typeof payload === "object" ? payload as Record<string, unknown> : {};
        const message = typeof record.message === "string" ? record.message : `HTTP ${response.status}`;
        throw new Error(message);
    }
    return payload as T;
}
