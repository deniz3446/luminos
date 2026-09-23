import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const dir = path.dirname(fileURLToPath(import.meta.url));
const source = fs.readFileSync(path.resolve(dir, '../src/pages/LogsPage.tsx'), 'utf8');
const failures = [];
for (const endpoint of ['/api/v1/logs', '/api/v1/logs/services', '/api/v1/logs/summary', '/api/v1/logs/health']) {
  if (!source.includes(endpoint)) failures.push(`missing log endpoint ${endpoint}`);
}
for (const marker of ['className="log-center-page"', 'LogsPage.css', 'type LogService', 'log-empty-state', 'log-api-error']) {
  if (!source.includes(marker)) failures.push(`missing logs UI contract: ${marker}`);
}
if (/services\?:string\[\]/.test(source)) failures.push('legacy string[] log service model remains');
if (failures.length) {
  console.error('LOGS_PAGE_CONTRACT=FAIL'); failures.forEach((item) => console.error(`- ${item}`)); process.exit(1);
}
console.log('LOGS_PAGE_CONTRACT=PASS');
