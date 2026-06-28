import { api } from "./client";

export async function getUsers() {
    const res = await api("/api/v1/users");

    return res.json();
}

export async function createUser(data: {
    username: string;
    email: string;
    password: string;
}) {
    const res = await api(
        "/api/v1/users",
        {
            method: "POST",
            body: JSON.stringify(data),
        },
    );

    return res.json();
}
