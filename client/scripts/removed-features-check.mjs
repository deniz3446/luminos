import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const client = path.resolve(here, "..");
const src = path.join(client, "src");

const failures = [];

function fail(message) {
  failures.push(message);
}

function read(rel) {
  return fs.readFileSync(path.join(client, rel), "utf8");
}

function walk(dir) {
  const result = [];

  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);

    if (entry.isDirectory()) {
      result.push(...walk(full));
      continue;
    }

    if (/\.(ts|tsx|css)$/.test(entry.name)) {
      result.push(full);
    }
  }

  return result;
}

const forbiddenPages = [
  "src/pages/MirrorPage.tsx",
  "src/pages/DockerManagerPage.tsx",
  "src/pages/CloudSyncPage.tsx",
  "src/pages/AutomationPage.tsx",
];

for (const rel of forbiddenPages) {
  if (fs.existsSync(path.join(client, rel))) {
    fail(`forbidden page still exists: ${rel}`);
  }
}

const app = read("src/App.tsx");
const layout = read("src/layouts/DashboardLayout.tsx");
const settings = read("src/pages/SettingsPage.tsx");
const health = read("src/pages/HealthPage.tsx");

const forbiddenRoutes = [
  "/mirror",
  "/docker",
  "/docker-manager",
  "/cloud-sync",
  "/automation",
];

for (const route of forbiddenRoutes) {
  if (app.includes(`path="${route}"`) || app.includes(`path='${route}'`)) {
    fail(`removed route still registered: ${route}`);
  }

  if (layout.includes(`"${route}"`) || layout.includes(`'${route}'`)) {
    fail(`removed navigation link still present: ${route}`);
  }
}

if (settings.includes("/cloud-sync")) {
  fail("Cloud Sync still present in Settings");
}

for (const token of [
  "/api/v1/docker",
  "/api/v1/cloud-sync",
  "/api/v1/cloud/accounts",
]) {
  if (health.includes(token)) {
    fail(`removed health dependency still present: ${token}`);
  }
}

const files = walk(src);

const forbiddenApis = [
  "/api/v1/mirror",
  "/api/v1/docker",
  "/api/v1/cloud-sync",
  "/api/v1/cloud/accounts",
  "/api/v1/automation",
];

const forbiddenUiPatterns = [
  /\bMirror Manager\b/i,
  /\bDocker Manager\b/i,
  /\bCloud Sync\b/i,
  /\bAutomation Center\b/i,
  /\bOtomasyon\b/i,
];

for (const file of files) {
  const text = fs.readFileSync(file, "utf8");
  const rel = path.relative(client, file);

  for (const api of forbiddenApis) {
    if (text.includes(api)) {
      fail(`${rel}: forbidden API reference ${api}`);
    }
  }

  for (const pattern of forbiddenUiPatterns) {
    if (pattern.test(text)) {
      fail(`${rel}: forbidden UI label ${pattern}`);
    }
  }
}

if (failures.length) {
  console.error("REMOVED_FEATURES_FRONTEND_CONTRACT=FAIL");

  for (const item of [...new Set(failures)]) {
    console.error(`- ${item}`);
  }

  process.exit(1);
}

console.log("removed_pages=PASS");
console.log("removed_routes=PASS");
console.log("removed_navigation=PASS");
console.log("removed_frontend_apis=PASS");
console.log("removed_ui_labels=PASS");
console.log("REMOVED_FEATURES_FRONTEND_CONTRACT=PASS");
