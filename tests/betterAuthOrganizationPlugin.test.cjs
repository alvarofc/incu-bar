const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const authPath = path.join(root, 'infra', 'cloudflare-gateway', 'src', 'auth.ts');
const workerPath = path.join(root, 'infra', 'cloudflare-gateway', 'src', 'index.ts');
const wranglerPath = path.join(root, 'infra', 'cloudflare-gateway', 'wrangler.toml');
const dashboardPath = path.join(root, 'infra', 'dashboard', 'index.html');

const authFile = fs.readFileSync(authPath, 'utf-8');
const workerFile = fs.readFileSync(workerPath, 'utf-8');
const wranglerFile = fs.readFileSync(wranglerPath, 'utf-8');
const dashboardFile = fs.readFileSync(dashboardPath, 'utf-8');

const authMarkers = [
  "organization()",
  'bearer()',
  'apiKey({',
  'enableSessionForAPIKeys: true',
  'runAuthMigrations',
];

authMarkers.forEach((marker) => {
  if (!authFile.includes(marker)) {
    throw new Error(`Auth config missing marker: ${marker}`);
  }
});

const workerMarkers = [
  '/api/auth',
  '/v1/dashboard/organizations',
  'createAuth(env)',
  'resolveTenantFromAuth',
];

workerMarkers.forEach((marker) => {
  if (!workerFile.includes(marker)) {
    throw new Error(`Worker missing Better Auth marker: ${marker}`);
  }
});

const wranglerMarkers = ['AUTH_DB', 'AUTH_BASE_URL', 'AUTH_TRUSTED_ORIGINS', 'nodejs_compat'];
wranglerMarkers.forEach((marker) => {
  if (!wranglerFile.includes(marker)) {
    throw new Error(`Wrangler config missing marker: ${marker}`);
  }
});

const dashboardMarkers = [
  'Better Auth email',
  'organization-select',
  '/api/auth/sign-in/email',
  '/v1/dashboard/organizations',
  'Create organization',
];

dashboardMarkers.forEach((marker) => {
  if (!dashboardFile.includes(marker)) {
    throw new Error(`Dashboard missing Better Auth marker: ${marker}`);
  }
});

console.log('Better Auth organization integration checks passed.');
