import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..');
const pagePath = path.join(root, 'src/pages/SystemUpdatesPage.tsx');
const cssPath = path.join(root, 'src/pages/SystemUpdatesPage.css');
const routesDir = path.resolve(root, '../server/src/routes');
const source = fs.readFileSync(pagePath, 'utf8');
const failures = [];

for (const marker of [
  'import "./SystemUpdatesPage.css"',
  'className="update-center-page"',
  'Promise.allSettled',
  'Update Agent erişilemiyor',
  'uc-summary-grid',
  'uc-tabs',
]) {
  if (!source.includes(marker)) failures.push(`missing: ${marker}`);
}

const endpoints = [
  '/api/v1/system/update/status',
  '/api/v1/system/update/releases',
  '/api/v1/system/update/packages',
  '/api/v1/system/update/history',
  '/api/v1/system/update/upload',
  '/api/v1/system/update/verify',
  '/api/v1/system/update/install',
  '/api/v1/system/update/rollback',
];
const routeText = fs.readdirSync(routesDir)
  .filter((name) => name.endsWith('.rs'))
  .map((name) => fs.readFileSync(path.join(routesDir, name), 'utf8'))
  .join('\n');
for (const endpoint of endpoints) {
  if (!source.includes(endpoint)) failures.push(`frontend endpoint missing: ${endpoint}`);
  if (!routeText.includes(`"${endpoint}"`)) failures.push(`backend route missing: ${endpoint}`);
}

for (const marker of ['/api/v1/system/reboot', '/api/v1/system/shutdown', 'Mirror', 'Docker', 'Cloud Sync', 'Automation']) {
  if (source.includes(marker)) failures.push(`forbidden stale/retired reference: ${marker}`);
}

if (!fs.existsSync(cssPath)) failures.push('SystemUpdatesPage.css missing');
else {
  const css = fs.readFileSync(cssPath, 'utf8');
  for (const marker of ['.update-center-page', '.uc-summary-grid', '.uc-panel', '.uc-status-pill']) {
    if (!css.includes(marker)) failures.push(`css missing: ${marker}`);
  }
}

if (failures.length) {
  console.error('SYSTEM_UPDATES_CONTRACT=FAIL');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log('SYSTEM_UPDATES_CONTRACT=PASS');
