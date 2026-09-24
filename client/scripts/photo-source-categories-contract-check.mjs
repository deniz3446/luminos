import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const photos = fs.readFileSync(path.join(root, "src/pages/PhotosPage.tsx"), "utf8");

const required = [
  'value: "whatsapp"',
  'label: "💬 WhatsApp"',
  'data-source-filter=',
  'aria-pressed=',
  'setSourceFilter(option.value)',
];

const missing = required.filter((item) => !photos.includes(item));
if (missing.length) {
  console.error("PHOTO_SOURCE_UI_CONTRACT=FAIL missing=" + missing.join(","));
  process.exit(1);
}

console.log("PHOTO_SOURCE_UI_CONTRACT=PASS");
