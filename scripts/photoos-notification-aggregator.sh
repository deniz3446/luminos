#!/usr/bin/env bash

set -euo pipefail

STATE_DIR="/var/lib/photoos/runtime/notification-producers"

ENV_OUTPUT="$STATE_DIR/notifications-aggregate.env"
JSON_OUTPUT="$STATE_DIR/notifications-aggregate.json"

ENV_TEMP="${ENV_OUTPUT}.tmp.$$"
JSON_TEMP="${JSON_OUTPUT}.tmp.$$"

trap 'rm -f "$ENV_TEMP" "$JSON_TEMP"' EXIT

mkdir -p "$STATE_DIR"

python3 - \
    "$STATE_DIR" \
    "$ENV_TEMP" \
    "$JSON_TEMP" <<'PY'
import json
import os
import re
import sys
import time
from pathlib import Path
from typing import Dict, Any


STATE_DIR = Path(sys.argv[1])
ENV_OUTPUT = Path(sys.argv[2])
JSON_OUTPUT = Path(sys.argv[3])

NOW = int(time.time())

PRODUCER_VERSION = "5.6"

PRODUCERS = [
    {
        "name": "storage",
        "file": "storage-state.env",
        "prefix": "STORAGE",
        "default_title": "Depolama sistemi",
        "max_age": 180,
    },
    {
        "name": "raid",
        "file": "raid-state.env",
        "prefix": "RAID",
        "default_title": "RAID sistemi",
        "max_age": 180,
    },
    {
        "name": "backup",
        "file": "backup-state.env",
        "prefix": "BACKUP",
        "default_title": "Yedekleme sistemi",
        "max_age": 180,
    },
    {
        "name": "update",
        "file": "update-state.env",
        "prefix": "UPDATE",
        "default_title": "Güncelleme sistemi",
        "max_age": 1200,
    },
]

LEVEL_NORMALIZATION = {
    "success": "success",
    "healthy": "success",
    "ok": "success",
    "info": "info",
    "running": "info",
    "warning": "warning",
    "warn": "warning",
    "error": "error",
    "failed": "error",
    "failure": "error",
    "critical": "critical",
    "danger": "critical",
}

LEVEL_WEIGHT = {
    "success": 0,
    "info": 1,
    "warning": 2,
    "error": 3,
    "critical": 4,
}

LEVEL_TURKISH = {
    "success": "Başarılı",
    "info": "Bilgi",
    "warning": "Uyarı",
    "error": "Hata",
    "critical": "Kritik",
}


def parse_env_file(path: Path) -> Dict[str, str]:
    result: Dict[str, str] = {}

    if not path.exists():
        return result

    try:
        content = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return result

    for raw_line in content.splitlines():
        line = raw_line.strip()

        if not line or line.startswith("#") or "=" not in line:
            continue

        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip()

        if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", key):
            continue

        if len(value) >= 2 and value[0] == '"' and value[-1] == '"':
            value = value[1:-1]
            value = value.replace(r"\\", "\\")
            value = value.replace(r"\"", '"')

        result[key] = value

    return result


def first_value(data: Dict[str, str], *keys: str, default: str = "") -> str:
    for key in keys:
        value = data.get(key, "").strip()

        if value:
            return value

    return default


def to_int(value: Any, default: int = 0) -> int:
    try:
        return int(float(str(value).strip()))
    except (TypeError, ValueError):
        return default


def normalize_level(level: str, status: str) -> str:
    level_lower = (level or "").strip().lower()
    status_lower = (status or "").strip().lower()

    if level_lower in LEVEL_NORMALIZATION:
        return LEVEL_NORMALIZATION[level_lower]

    if status_lower in LEVEL_NORMALIZATION:
        return LEVEL_NORMALIZATION[status_lower]

    if status_lower in {"inactive", "idle", "completed"}:
        return "success"

    return "info"


def env_escape(value: Any) -> str:
    text = str(value if value is not None else "")
    text = text.replace("\\", "\\\\")
    text = text.replace('"', '\\"')
    text = text.replace("\r", " ")
    text = text.replace("\n", " ")
    return text


items = []

for producer in PRODUCERS:
    name = producer["name"]
    prefix = producer["prefix"]
    path = STATE_DIR / producer["file"]

    exists = path.is_file()
    data = parse_env_file(path) if exists else {}

    generated_at = to_int(data.get("GENERATED_AT"), 0)
    age_seconds = NOW - generated_at if generated_at > 0 else None

    file_fresh = (
        exists
        and generated_at > 0
        and age_seconds is not None
        and age_seconds <= producer["max_age"]
    )

    if not exists:
        item = {
            "id": f"{name}_producer_missing",
            "producer": name,
            "producer_version": "",
            "status": "critical",
            "level": "critical",
            "code": f"{name}_producer_missing",
            "title": f"{producer['default_title']} verisi bulunamadı",
            "message": f"{producer['file']} dosyası bulunamadı.",
            "error": f"Eksik producer state dosyası: {path}",
            "generated_at": None,
            "age_seconds": None,
            "fresh": False,
            "state_file": str(path),
            "active": True,
        }

        items.append(item)
        continue

    if not file_fresh:
        if generated_at <= 0:
            stale_message = (
                f"{producer['default_title']} state dosyasında geçerli "
                "GENERATED_AT bilgisi bulunamadı."
            )
        else:
            stale_message = (
                f"{producer['default_title']} verisi güncel değil. "
                f"Son veri {age_seconds} saniye önce üretildi."
            )

        item = {
            "id": f"{name}_producer_stale",
            "producer": name,
            "producer_version": data.get("PRODUCER_VERSION", ""),
            "status": "warning",
            "level": "warning",
            "code": f"{name}_producer_stale",
            "title": f"{producer['default_title']} verisi güncel değil",
            "message": stale_message,
            "error": "",
            "generated_at": generated_at if generated_at > 0 else None,
            "age_seconds": age_seconds,
            "fresh": False,
            "state_file": str(path),
            "active": True,
        }

        items.append(item)
        continue

    status = first_value(
        data,
        f"{prefix}_STATUS",
        default="unknown",
    )

    raw_level = first_value(
        data,
        f"{prefix}_LEVEL",
        default="info",
    )

    level = normalize_level(raw_level, status)

    code = first_value(
        data,
        f"{prefix}_CODE",
        default=f"{name}_unknown",
    )

    title = first_value(
        data,
        f"{prefix}_TITLE",
        default=producer["default_title"],
    )

    message = first_value(
        data,
        f"{prefix}_NOTIFICATION_MESSAGE",
        f"{prefix}_MESSAGE",
        default=f"{producer['default_title']} durumu alındı.",
    )

    error = first_value(
        data,
        f"{prefix}_ERROR",
        default="",
    )

    active = level in {"info", "warning", "error", "critical"}

    item = {
        "id": code,
        "producer": name,
        "producer_version": data.get("PRODUCER_VERSION", ""),
        "status": status,
        "level": level,
        "code": code,
        "title": title,
        "message": message,
        "error": error,
        "generated_at": generated_at,
        "age_seconds": age_seconds,
        "fresh": True,
        "state_file": str(path),
        "active": active,
    }

    items.append(item)


items.sort(
    key=lambda item: (
        -LEVEL_WEIGHT.get(item["level"], 1),
        item["producer"],
    )
)

counts = {
    "total": len(items),
    "success": sum(1 for item in items if item["level"] == "success"),
    "info": sum(1 for item in items if item["level"] == "info"),
    "warning": sum(1 for item in items if item["level"] == "warning"),
    "error": sum(1 for item in items if item["level"] == "error"),
    "critical": sum(1 for item in items if item["level"] == "critical"),
    "active": sum(1 for item in items if item["active"]),
    "fresh": sum(1 for item in items if item["fresh"]),
    "stale": sum(1 for item in items if not item["fresh"]),
}

highest_level = "success"

for candidate in ["critical", "error", "warning", "info", "success"]:
    if counts[candidate] > 0:
        highest_level = candidate
        break

if counts["critical"] > 0:
    overall_status = "critical"
    overall_code = "system_critical"
    overall_title = "PhotoOS kritik sistem uyarısı"
    overall_message = f"{counts['critical']} kritik sistem sorunu bulundu."

elif counts["error"] > 0:
    overall_status = "error"
    overall_code = "system_error"
    overall_title = "PhotoOS sistem hatası"
    overall_message = f"{counts['error']} sistem hatası bulundu."

elif counts["warning"] > 0:
    overall_status = "warning"
    overall_code = "system_warning"
    overall_title = "PhotoOS sistem uyarısı"
    overall_message = f"{counts['warning']} sistem uyarısı bulundu."

elif counts["info"] > 0:
    overall_status = "info"
    overall_code = "system_information"
    overall_title = "PhotoOS sistem bilgisi"
    overall_message = f"{counts['info']} bilgilendirme bildirimi bulunuyor."

else:
    overall_status = "healthy"
    overall_code = "system_healthy"
    overall_title = "PhotoOS sistemi sağlıklı"
    overall_message = "Tüm PhotoOS bileşenleri normal çalışıyor."

problem_items = [
    item
    for item in items
    if item["level"] in {"warning", "error", "critical"}
]

informational_items = [
    item
    for item in items
    if item["level"] == "info"
]

healthy_items = [
    item
    for item in items
    if item["level"] == "success"
]

summary = {
    "producer_name": "notification_aggregator",
    "producer_version": PRODUCER_VERSION,
    "generated_at": NOW,
    "overall": {
        "status": overall_status,
        "level": highest_level,
        "code": overall_code,
        "title": overall_title,
        "message": overall_message,
    },
    "counts": counts,
    "notifications": items,
    "problems": problem_items,
    "information": informational_items,
    "healthy": healthy_items,
}

JSON_OUTPUT.write_text(
    json.dumps(
        summary,
        ensure_ascii=False,
        indent=2,
        sort_keys=False,
    )
    + "\n",
    encoding="utf-8",
)

env_lines = [
    'PRODUCER_NAME="notification_aggregator"',
    f'PRODUCER_VERSION="{PRODUCER_VERSION}"',
    f"GENERATED_AT={NOW}",
    f'AGGREGATE_STATUS="{env_escape(overall_status)}"',
    f'AGGREGATE_LEVEL="{env_escape(highest_level)}"',
    f'AGGREGATE_CODE="{env_escape(overall_code)}"',
    f'AGGREGATE_TITLE="{env_escape(overall_title)}"',
    f'AGGREGATE_NOTIFICATION_MESSAGE="{env_escape(overall_message)}"',
    f"AGGREGATE_TOTAL_COUNT={counts['total']}",
    f"AGGREGATE_ACTIVE_COUNT={counts['active']}",
    f"AGGREGATE_SUCCESS_COUNT={counts['success']}",
    f"AGGREGATE_INFO_COUNT={counts['info']}",
    f"AGGREGATE_WARNING_COUNT={counts['warning']}",
    f"AGGREGATE_ERROR_COUNT={counts['error']}",
    f"AGGREGATE_CRITICAL_COUNT={counts['critical']}",
    f"AGGREGATE_FRESH_COUNT={counts['fresh']}",
    f"AGGREGATE_STALE_COUNT={counts['stale']}",
    f'AGGREGATE_JSON_FILE="{env_escape(str(STATE_DIR / "notifications-aggregate.json"))}"',
]

for item in items:
    prefix = item["producer"].upper()

    env_lines.extend(
        [
            f'{prefix}_AGGREGATED_STATUS="{env_escape(item["status"])}"',
            f'{prefix}_AGGREGATED_LEVEL="{env_escape(item["level"])}"',
            f'{prefix}_AGGREGATED_CODE="{env_escape(item["code"])}"',
            f'{prefix}_AGGREGATED_TITLE="{env_escape(item["title"])}"',
            f'{prefix}_AGGREGATED_MESSAGE="{env_escape(item["message"])}"',
            f"{prefix}_AGGREGATED_FRESH={1 if item['fresh'] else 0}",
            f"{prefix}_AGGREGATED_ACTIVE={1 if item['active'] else 0}",
        ]
    )

ENV_OUTPUT.write_text(
    "\n".join(env_lines) + "\n",
    encoding="utf-8",
)

print("Notification Aggregator kontrolü tamamlandı.")
print(f"Durum: {overall_status}")
print(f"Kod: {overall_code}")
print(f"En yüksek seviye: {highest_level}")
print(f"Toplam producer: {counts['total']}")
print(f"Aktif bildirim: {counts['active']}")
print(f"Uyarı: {counts['warning']}")
print(f"Hata: {counts['error']}")
print(f"Kritik: {counts['critical']}")
PY

chmod 0644 "$ENV_TEMP"
chmod 0644 "$JSON_TEMP"

# Backend de ayni JSON hedefini yeniledigi icin root aggregator\n# hedef JSON dosyasini photoos kullanicisina devreder.\nif id photoos >/dev/null 2>&1; then\n    chown photoos:photoos "$JSON_TEMP"\nfi\n
mv -f "$ENV_TEMP" "$ENV_OUTPUT"
mv -f "$JSON_TEMP" "$JSON_OUTPUT"

echo "ENV çıktı: $ENV_OUTPUT"
echo "JSON çıktı: $JSON_OUTPUT"
