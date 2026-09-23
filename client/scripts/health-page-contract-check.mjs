import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const dir = path.dirname(fileURLToPath(import.meta.url));
const clientRoot = path.resolve(dir, '..');
const source = fs.readFileSync(path.join(clientRoot, 'src/pages/HealthPage.tsx'), 'utf8');
const failures = [];

const required = [
  '/api/v1/system/health',
  '/api/v1/system/info',
  '/api/v1/storage/health',
  '/api/v1/storage/disks',
  '/api/v1/raid/status',
  '/api/v1/backups/status',
  '/api/v1/notifications/summary',
  '/api/v1/logs/summary',
];
for (const endpoint of required) if (!source.includes(endpoint)) failures.push(`missing retained health endpoint ${endpoint}`);
for (const marker of ['className="health-center"', 'HealthPage.css', 'Bilgi alınamıyor', 'Sağlık puanı']) {
  if (!source.includes(marker)) failures.push(`missing health UI contract: ${marker}`);
}
if (/Mirror|Docker|Cloud Sync|Automation|cloud-sync|\/mirror|\/docker|\/automation/i.test(source)) failures.push('retired health feature reference present');

if (failures.length) {
  console.error('HEALTH_PAGE_CONTRACT=FAIL');
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log('HEALTH_PAGE_CONTRACT=PASS');
