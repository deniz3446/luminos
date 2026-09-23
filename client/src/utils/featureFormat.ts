export function formatBytes(value?: number | null): string {
    const bytes = Number(value ?? 0);
    if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const index = Math.min(
        Math.floor(Math.log(bytes) / Math.log(1024)),
        units.length - 1,
    );
    return `${(bytes / 1024 ** index).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

export function formatDate(value?: string | number | null): string {
    if (value === null || value === undefined || value === "") return "—";
    const date =
        typeof value === "number" ? new Date(value * 1000) : new Date(value);
    return Number.isNaN(date.getTime())
        ? String(value)
        : date.toLocaleString("tr-TR");
}
