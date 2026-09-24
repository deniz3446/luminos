from pathlib import Path

root = Path(__file__).resolve().parents[1]
server = (root / "server/src/handlers/photos.rs").read_text(encoding="utf-8")

required = [
    "pub source_type: Option<String>",
    "pub media_type: Option<String>",
    '"whatsapp"',
    '"whatsapp_received"',
    '"whatsapp_sent"',
    "normalize_source_filter",
    "normalize_media_filter",
]

missing = [item for item in required if item not in server]
if missing:
    raise SystemExit("PHOTO_SOURCE_BACKEND_CONTRACT=FAIL missing=" + ",".join(missing))

print("PHOTO_SOURCE_BACKEND_CONTRACT=PASS")
