const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const workerPath = path.join(root, 'infra', 'cloudflare-gateway', 'src', 'index.ts');
const dashboardPath = path.join(root, 'infra', 'dashboard', 'index.html');

const worker = fs.readFileSync(workerPath, 'utf-8');
const dashboard = fs.readFileSync(dashboardPath, 'utf-8');

const workerMarkers = [
  '/v1/dashboard/overview',
  '/v1/dashboard/summary',
  '/v1/dashboard/daily',
  '/v1/dashboard/providers',
  '/v1/dashboard/employees',
  'dashboardSummarySql',
  'dashboardDailySql',
  'dashboardProvidersSql',
  'dashboardEmployeesSql',
];

for (const marker of workerMarkers) {
  if (!worker.includes(marker)) {
    throw new Error(`Worker missing dashboard marker: ${marker}`);
  }
}

const dashboardMarkers = [
  'IncuBar Employer Intelligence',
  'id="daily-chart"',
  'id="provider-chart"',
  'id="employee-table-body"',
  '/v1/dashboard/overview',
  'Auto refresh: Off',
];

for (const marker of dashboardMarkers) {
  if (!dashboard.includes(marker)) {
    throw new Error(`Dashboard missing marker: ${marker}`);
  }
}

console.log('Employer dashboard parity checks passed.');
