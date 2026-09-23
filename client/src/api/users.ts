import { apiJson } from "./client";

export type UserSummary = {
    id: number;
    username: string;
    created_at: string;
};

export type CreateUserInput = {
    username: string;
    email: string;
    password: string;
};

export type ApiMessage = {
    success: boolean;
    message: string;
};

export async function getUsers(): Promise<UserSummary[]> {
    return apiJson<UserSummary[]>("/api/v1/users");
}

export async function createUser(data: CreateUserInput): Promise<ApiMessage> {
    return apiJson<ApiMessage>("/api/v1/users", {
        method: "POST",
        body: JSON.stringify(data),
    });
}
