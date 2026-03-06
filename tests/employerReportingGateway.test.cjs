const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const typesPath = path.join(root, 'src', 'lib', 'types.ts');
const providersPath = path.join(root, 'src', 'lib', 'providers.ts');
const settingsStorePath = path.join(root, 'src', 'stores', 'settingsStore.ts');
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const appPath = path.join(root, 'src', 'App.tsx');
const commandsPath = path.join(root, 'src-tauri', 'src', 'commands', 'mod.rs');
const libPath = path.join(root, 'src-tauri', 'src', 'lib.rs');
const reportingModulePath = path.join(root, 'src-tauri', 'src', 'reporting', 'mod.rs');
const workerPath = path.join(root, 'infra', 'cloudflare-gateway', 'src', 'index.ts');
const datasourcePath = path.join(root, 'infra', 'tinybird', 'incubar_usage_events.datasource');

const files = {
  types: fs.readFileSync(typesPath, 'utf-8'),
  providers: fs.readFileSync(providersPath, 'utf-8'),
  settingsStore: fs.readFileSync(settingsStorePath, 'utf-8'),
  settingsPanel: fs.readFileSync(settingsPanelPath, 'utf-8'),
  app: fs.readFileSync(appPath, 'utf-8'),
  commands: fs.readFileSync(commandsPath, 'utf-8'),
  lib: fs.readFileSync(libPath, 'utf-8'),
  reporting: fs.readFileSync(reportingModulePath, 'utf-8'),
  worker: fs.readFileSync(workerPath, 'utf-8'),
  datasource: fs.readFileSync(datasourcePath, 'utf-8'),
};

const requiredMarkers = [
  { name: 'employerReportingEnabled', sources: [files.types, files.providers, files.settingsStore, files.settingsPanel, files.app] },
  { name: 'employerReportingGatewayUrl', sources: [files.types, files.providers, files.settingsStore, files.settingsPanel, files.app] },
  { name: 'employerReportingEmployeeId', sources: [files.types, files.providers, files.settingsStore, files.settingsPanel, files.app] },
  { name: 'set_employer_reporting_config', sources: [files.app, files.commands, files.lib] },
  { name: 'send_employer_usage_report', sources: [files.app, files.commands, files.lib] },
  { name: 'queue_close_report_once', sources: [files.lib, files.reporting] },
  { name: 'KEYCHAIN_GATEWAY_KEY', sources: [files.reporting] },
  { name: 'Cloudflare', sources: [files.settingsPanel, files.worker] },
  { name: 'tenant_id', sources: [files.worker, files.datasource] },
  { name: 'incubar_usage_events', sources: [files.worker, files.datasource] },
  { name: 'data-testid="employer-reporting-settings"', sources: [files.settingsPanel] },
  { name: '/v1/usage-report', sources: [files.worker] },
];

requiredMarkers.forEach(({ name, sources }) => {
  if (!sources.some((source) => source.includes(name))) {
    throw new Error(`Employer reporting marker missing: ${name}`);
  }
});

if (!files.app.includes('employerOpenReportSentRef.current = false;')) {
  throw new Error('Employer reporting open-send guard reset is missing.');
}

console.log('Employer reporting gateway checks passed.');
