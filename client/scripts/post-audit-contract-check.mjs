import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..');
const failures = [];
const read = (relative) => fs.readFileSync(path.resolve(root, relative), 'utf8');

const dashboard = read('src/pages/DashboardPage.tsx');
const health = read('src/pages/HealthPage.tsx');
const systemRoutes = read('../server/src/routes/system.rs');
const systemHandler = read('../server/src/handlers/system.rs');

if (!dashboard.includes('/api/v1/system/metrics')) failures.push('dashboard no longer requests system metrics');
if (!systemRoutes.includes('"/api/v1/system/metrics"')) failures.push('backend system metrics route missing');
if (!systemRoutes.includes('system_metrics')) failures.push('system metrics handler not wired');

if (!health.includes('"neutral"')) failures.push('health neutral state missing');
if (!/configured \? \(healthy \? "success" : "warning"\) : "neutral"/.test(health)) {
  failures.push('unconfigured RAID must be neutral, not unavailable');
}
if (!health.includes('tones.filter((tone) => tone === "unknown").length')) failures.push('unknown health count contract missing');

if (!systemHandler.includes('product: "PhotoOS Server"')) failures.push('system identity must be PhotoOS Server');
if (!systemHandler.includes('settings.release.version.clone()')) failures.push('system info must use configured release version');
if (systemHandler.includes('product: "LuminOS Server"') || systemHandler.includes('version: "0.1.0-dev"')) {
  failures.push('legacy LuminOS/dev identity remains');
}

if (failures.length) {
  console.error('POST_AUDIT_CONTRACT=FAIL');
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log('POST_AUDIT_CONTRACT=PASS');
