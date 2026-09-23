#!/usr/bin/env bash
set -Eeuo pipefail

export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

OUT="/var/lib/photoos/runtime/disk-health.json"
TMP="${OUT}.tmp"

mkdir -p "$(dirname "$OUT")"

LIVE_SOURCE="$(findmnt -n -o SOURCE /run/live/medium 2>/dev/null || true)"
LIVE_DISK=""

if [[ "$LIVE_SOURCE" =~ ^/dev/ ]]; then
    LIVE_PK="$(lsblk -no PKNAME "$LIVE_SOURCE" 2>/dev/null || true)"
    [[ -n "$LIVE_PK" ]] && LIVE_DISK="/dev/$LIVE_PK"
fi

python3 - "$OUT" "$TMP" "$LIVE_DISK" <<'PY'
import json
import subprocess
import sys
import time
from pathlib import Path

out = Path(sys.argv[1])
tmp = Path(sys.argv[2])
live_disk = sys.argv[3].strip()

def run(*args):
    return subprocess.run(
        args,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )

ls = run(
    "lsblk",
    "-J",
    "-b",
    "-d",
    "-o",
    "NAME,PATH,SIZE,MODEL,SERIAL,TYPE,TRAN,ROTA"
)

try:
    block = json.loads(ls.stdout or "{}")
except Exception:
    block = {"blockdevices": []}

items = []

for dev in block.get("blockdevices", []):
    if dev.get("type") != "disk":
        continue

    path = dev.get("path") or f"/dev/{dev.get('name','')}"

    if not path or path == live_disk:
        continue

    result = {
        "device": path,
        "name": dev.get("name"),
        "size_bytes": dev.get("size"),
        "model": (dev.get("model") or "").strip(),
        "serial": (dev.get("serial") or "").strip(),
        "transport": dev.get("tran"),
        "rotational": dev.get("rota"),
        "smart_available": False,
        "smart_status": "unknown",
        "temperature_c": None,
        "power_on_hours": None,
        "health_passed": None,
        "error": None,
    }

    smart = run("smartctl", "-a", "-j", path)

    try:
        data = json.loads(smart.stdout or "{}")

        result["smart_available"] = bool(
            data.get("smart_support", {}).get("available", False)
            or data.get("smart_support", {}).get("enabled", False)
            or data.get("smart_status") is not None
        )

        passed = data.get("smart_status", {}).get("passed")
        result["health_passed"] = passed

        if passed is True:
            result["smart_status"] = "healthy"
        elif passed is False:
            result["smart_status"] = "critical"
        elif result["smart_available"]:
            result["smart_status"] = "available"

        temp = data.get("temperature", {}).get("current")
        if isinstance(temp, (int, float)):
            result["temperature_c"] = temp

        poh = data.get("power_on_time", {}).get("hours")
        if isinstance(poh, (int, float)):
            result["power_on_hours"] = poh

        if smart.returncode not in (0, 2):
            err = (smart.stderr or "").strip()
            if err:
                result["error"] = err[:500]

    except Exception as exc:
        result["error"] = f"SMART JSON parse error: {exc}"

    items.append(result)

healthy = sum(1 for x in items if x["health_passed"] is True)
critical = sum(1 for x in items if x["health_passed"] is False)
warning = sum(
    1 for x in items
    if x["health_passed"] is None and x["smart_available"]
)

payload = {
    "generated_at": int(time.time()),
    "producer": "photoos-disk-health",
    "producer_version": "1.0",
    "status": (
        "critical"
        if critical
        else "warning"
        if warning
        else "healthy"
    ),
    "summary": {
        "total": len(items),
        "healthy": healthy,
        "warning": warning,
        "critical": critical,
    },
    "live_disk_excluded": live_disk or None,
    "disks": items,
}

tmp.write_text(
    json.dumps(payload, ensure_ascii=False, indent=2),
    encoding="utf-8",
)

tmp.replace(out)
PY

chmod 0644 "$OUT"

logger -t photoos-disk-health \
    "Disk Health cache güncellendi: $OUT"
