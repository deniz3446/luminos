const API_URL = "http://192.168.1.41:8080";

export async function api(
    path: string,
    options?: RequestInit,
) {
    const token = localStorage.getItem("token");

    const headers = new Headers(options?.headers);

    if (!headers.has("Content-Type")) {
        headers.set("Content-Type", "application/json");
    }

    if (token) {
        headers.set(
            "Authorization",
            `Bearer ${token}`,
        );
    }

    const response = await fetch(
        API_URL + path,
        {
            ...options,
            headers,
        },
    );

    return response;
}
