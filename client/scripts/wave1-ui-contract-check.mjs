import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const clientRoot = path.resolve(scriptDir, '..', 'src');
const repoRoot = path.resolve(scriptDir, '..', '..');
const serverRoot = path.join(repoRoot, 'server', 'src');

const read = (relative) => fs.readFileSync(path.join(clientRoot, relative), 'utf8');
const walk = (dir) => fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
  const full = path.join(dir, entry.name);
  return entry.isDirectory() ? walk(full) : [full];
});

const backendSource = walk(serverRoot)
  .filter((file) => file.endsWith('.rs'))
  .map((file) => fs.readFileSync(file, 'utf8'))
  .join('\n');

const failures = [];
const requireText = (source, needle, message) => {
  if (!source.includes(needle)) failures.push(message);
};
const forbid = (source, pattern, message) => {
  if (pattern.test(source)) failures.push(message);
};

const layout = read('layouts/DashboardLayout.tsx');
const layoutCss = read('layouts/DashboardLayout.css');
const indexCss = read('index.css');
const themeCss = read('styles/photoos-theme.css');

for (const cls of ['photoos-shell', 'photoos-sidebar', 'photoos-topbar', 'photoos-content', 'nav-section']) {
  requireText(layout, cls, `shared shell missing class: ${cls}`);
}
for (const token of ['--aurora-bg', '--aurora-surface', '--aurora-border', '--aurora-text', '--aurora-cyan']) {
  if (!layoutCss.includes(token) && !indexCss.includes(token) && !themeCss.includes(token)) failures.push(`professional theme token missing: ${token}`);
}

const pages = [
  ['pages/HealthPage.tsx', 'health-center'],
  ['pages/LogsPage.tsx', 'log-center-page'],
  ['pages/ControlPage.tsx', 'control-center-page'],
  ['pages/SystemUpdatesPage.tsx', 'update-center-page'],
];

const retired = /\b(Mirror|Docker Manager|Cloud Sync|Automation|cloud-sync|\/mirror|\/docker|\/automation)\b/i;
const literalApi = /["'`]((?:\/api\/v1\/)[A-Za-z0-9_./{}-]+)/g;
const allowPrefix = ['/api/v1/photos/file/', '/api/v1/photos/thumb/'];

for (const [file, rootClass] of pages) {
  const source = read(file);
  requireText(source, rootClass, `${file} missing stable root class ${rootClass}`);
  forbid(source, retired, `${file} references retired feature`);

  for (const match of source.matchAll(literalApi)) {
    const endpoint = match[1].split('?')[0];
    if (allowPrefix.some((prefix) => endpoint.startsWith(prefix))) continue;
    if (!backendSource.includes(`\"${endpoint}\"`)) {
      failures.push(`${file} endpoint not found in backend routes: ${endpoint}`);
    }
  }
}

for (const label of ['Mirror', 'Docker', 'Cloud Sync', 'Automation']) {
  if (layout.includes(label)) failures.push(`retired navigation label present: ${label}`);
}

if (failures.length) {
  console.error('WAVE1_UI_CONTRACT=FAIL');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('WAVE1_UI_CONTRACT=PASS');
