import { useCallback, useEffect, useMemo, useState } from "react";

import { api } from "../api/client";
import "./StorageTelemetryPanel.css";

export type StorageTelemetryDisk = {
  device: string;
  label: string;
};

type MetricPoint = {
  timestamp: number;
  temperature_celsius: number | null;
  total_read_bytes: number;
  total_write_bytes: number;
  read_bytes_per_second: number;
  write_bytes_per_second: number;
  read_iops: number;
  write_iops: number;
  busy_percent: number;
};

type DeviceMetrics = {
  device: string;
  model: string;
  current: MetricPoint | null;
  history: MetricPoint[];
};

type MetricsResponse = {
  checked_at: string;
  sample_interval_seconds: number;
  retention_days: number;
  devices: DeviceMetrics[];
};

function formatBytes(value: number) {
  if (!Number.isFinite(value) || value <= 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let amount = value;
  let unit = 0;

  while (amount >= 1024 && unit < units.length - 1) {
    amount /= 1024;
    unit += 1;
  }

  return `${amount.toLocaleString("tr-TR", {
    maximumFractionDigits: amount >= 100 ? 0 : amount >= 10 ? 1 : 2,
  })} ${units[unit]}`;
}

function formatRate(value: number | undefined) {
  return `${formatBytes(value ?? 0)}/sn`;
}

function chartPoints(history: MetricPoint[]) {
  const temperatures = history.filter(
    (point): point is MetricPoint & { temperature_celsius: number } =>
      typeof point.temperature_celsius === "number",
  );

  if (temperatures.length === 0) {
    return { points: "", min: null, max: null };
  }

  const values = temperatures.map((point) => point.temperature_celsius);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const chartMin = Math.min(25, min - 2);
  const chartMax = Math.max(55, max + 2);
  const range = Math.max(1, chartMax - chartMin);

  const points = temperatures
    .map((point, index) => {
      const x = (index / Math.max(1, temperatures.length - 1)) * 600;
      const y = 145 - ((point.temperature_celsius - chartMin) / range) * 125;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");

  const visiblePoints =
    temperatures.length === 1
      ? `${points} 600,${points.split(",")[1]}`
      : points;

  return { points: visiblePoints, min, max };
}

function TelemetryCard(props: { label: string; metrics?: DeviceMetrics }) {
  const { label, metrics } = props;
  const current = metrics?.current;
  const chart = useMemo(
    () => chartPoints(metrics?.history ?? []),
    [metrics?.history],
  );

  return (
    <article className="storage-telemetry-card">
      <div className="storage-telemetry-head">
        <div>
          <span>{metrics?.device ?? "Disk"}</span>
          <h3>{label}</h3>
          <small>{metrics?.model || "Disk bilgisi bekleniyor"}</small>
        </div>

        <strong
          className={(current?.temperature_celsius ?? 0) >= 50 ? "hot" : ""}
        >
          {current?.temperature_celsius !== null &&
          current?.temperature_celsius !== undefined
            ? `${current.temperature_celsius}°C`
            : "—"}
        </strong>
      </div>

      <div className="storage-io-grid">
        <div>
          <small>Okuma</small>
          <strong>↓ {formatRate(current?.read_bytes_per_second)}</strong>
        </div>
        <div>
          <small>Yazma</small>
          <strong>↑ {formatRate(current?.write_bytes_per_second)}</strong>
        </div>
        <div>
          <small>IOPS</small>
          <strong>
            {(current?.read_iops ?? 0).toFixed(1)} /{" "}
            {(current?.write_iops ?? 0).toFixed(1)}
          </strong>
        </div>
        <div>
          <small>Disk yoğunluğu</small>
          <strong>%{(current?.busy_percent ?? 0).toFixed(1)}</strong>
        </div>
        <div>
          <small>Toplam okuma</small>
          <strong>{formatBytes(current?.total_read_bytes ?? 0)}</strong>
        </div>
        <div>
          <small>Toplam yazma</small>
          <strong>{formatBytes(current?.total_write_bytes ?? 0)}</strong>
        </div>
      </div>

      <div className="storage-temperature-chart">
        <div className="storage-chart-title">
          <div>
            <strong>Sıcaklık geçmişi</strong>
            <small>Son 24 saat · 5 dakikalık ortalama</small>
          </div>
          <span>
            {chart.min !== null ? `En az ${chart.min}°C` : "—"}
            {" · "}
            {chart.max !== null ? `En çok ${chart.max}°C` : "—"}
          </span>
        </div>

        {chart.points ? (
          <svg
            viewBox="0 0 600 160"
            role="img"
            aria-label={`${label} sıcaklık grafiği`}
          >
            <line x1="0" y1="20" x2="600" y2="20" />
            <line x1="0" y1="82" x2="600" y2="82" />
            <line x1="0" y1="145" x2="600" y2="145" />
            <polyline points={chart.points} />
          </svg>
        ) : (
          <div className="storage-chart-empty">
            İlk ölçümler toplanıyor. Grafik birkaç dakika içinde oluşacak.
          </div>
        )}
      </div>
    </article>
  );
}

export default function StorageTelemetryPanel(props: {
  disks: StorageTelemetryDisk[];
}) {
  const [data, setData] = useState<MetricsResponse | null>(null);
  const [error, setError] = useState("");

  const loadMetrics = useCallback(async () => {
    try {
      const response = await api(
        `/api/v1/storage/metrics?hours=24&ts=${Date.now()}`,
      );

      if (!response.ok) {
        throw new Error(`Disk I/O API hatası: ${response.status}`);
      }

      setData((await response.json()) as MetricsResponse);
      setError("");
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Disk ölçümleri alınamadı.",
      );
    }
  }, []);

  useEffect(() => {
    void loadMetrics();

    const timer = window.setInterval(() => {
      void loadMetrics();
    }, 30000);

    return () => window.clearInterval(timer);
  }, [loadMetrics]);

  if (props.disks.length === 0) {
    return null;
  }

  const lastCheck = data?.checked_at
    ? new Date(data.checked_at).toLocaleString("tr-TR")
    : "Ölçüm bekleniyor";

  return (
    <section className="storage-manager-panel storage-telemetry-panel">
      <div className="storage-panel-heading">
        <div>
          <span>Canlı disk ölçümleri</span>
          <h2>Sıcaklık Geçmişi ve Disk I/O</h2>
        </div>

        <div className="storage-telemetry-status">
          <i />
          {lastCheck}
        </div>
      </div>

      {error && <div className="storage-telemetry-error">⚠️ {error}</div>}

      <div className="storage-telemetry-grid">
        {props.disks.map((disk) => (
          <TelemetryCard
            key={disk.device}
            label={disk.label}
            metrics={data?.devices.find((item) => item.device === disk.device)}
          />
        ))}
      </div>
    </section>
  );
}
