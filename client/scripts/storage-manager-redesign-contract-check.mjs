import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const page = fs.readFileSync(path.join(root, "src/pages/StoragePage.tsx"), "utf8");
const css = fs.readFileSync(path.join(root, "src/pages/StoragePage.css"), "utf8");
const telemetryCss = fs.readFileSync(path.join(root, "src/pages/StorageTelemetryPanel.css"), "utf8");

const checks = [
  [page.includes("DEPOLAMA YÖNETİCİSİ"), "hero kicker"],
  [page.includes("Güvenli Depolama"), "hero safety status"],
  [page.includes("storage-manager-hero-icon"), "hero icon"],
  [page.includes("storage-disk-facts"), "disk facts grid"],
  [page.includes("Kullanılan"), "used capacity metric"],
  [page.includes("Disk Türü"), "disk type fact"],
  [page.includes("Tüm Diskler Sağlıklı"), "smart summary badge"],
  [css.includes("--storage-cyan"), "storage theme tokens"],
  [css.includes("storage-manager-shell-glow"), "shell glow treatment"],
  [telemetryCss.includes("storage-telemetry-card::before"), "telemetry accent"],
];

const failed = checks.filter(([ok]) => !ok).map(([, name]) => name);
if (failed.length) {
  console.error(`STORAGE_MANAGER_REFERENCE_CONTRACT=FAIL ${failed.join(", ")}`);
  process.exit(1);
}
console.log("STORAGE_MANAGER_REFERENCE_CONTRACT=PASS");
