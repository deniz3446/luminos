import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..');
const pagePath = path.join(root, 'src/pages/ControlPage.tsx');
const cssPath = path.join(root, 'src/pages/ControlPage.css');
const serverRoutes = path.resolve(root, '../server/src/routes');
const source = fs.readFileSync(pagePath, 'utf8');
const failures = [];

const required = [
  'import "./ControlPage.css"',
  'className="control-center-page"',
  '/api/v1/system/info',
  '/api/v1/system/health',
  '/api/v1/storage',
  '/api/v1/photos',
  '/api/v1/dashboard/stats',
  'Promise.allSettled',
  'Veri alınamıyor',
];
for (const marker of required) {
  if (!source.includes(marker)) failures.push(`missing: ${marker}`);
}

const forbidden = [
  '/api/v1/system/backfill-taken-at',
  '/api/v1/system/rescan-media',
  '/api/v1/system/generate-thumbnails',
  '/api/v1/system/clear-thumbs',
  'Mirror', 'Docker', 'Cloud Sync', 'Automation',
];
for (const marker of forbidden) {
  if (source.includes(marker)) failures.push(`forbidden stale/retired reference: ${marker}`);
}

if (!fs.existsSync(cssPath)) failures.push('ControlPage.css missing');
else {
  const css = fs.readFileSync(cssPath, 'utf8');
  for (const marker of ['.control-center-page', '.control-summary-grid', '.control-panel', '.control-status']) {
    if (!css.includes(marker)) failures.push(`css missing: ${marker}`);
  }
}

const routeText = fs.readdirSync(serverRoutes)
  .filter((name) => name.endsWith('.rs'))
  .map((name) => fs.readFileSync(path.join(serverRoutes, name), 'utf8'))
  .join('\n');
for (const endpoint of ['/api/v1/system/info','/api/v1/system/health','/api/v1/storage','/api/v1/photos','/api/v1/dashboard/stats']) {
  if (!routeText.includes(`"${endpoint}"`)) failures.push(`backend route missing: ${endpoint}`);
}

if (failures.length) {
  console.error('CONTROL_PAGE_CONTRACT=FAIL');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log('CONTROL_PAGE_CONTRACT=PASS');
