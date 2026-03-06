// Cloudflare Worker gateway for IncuBar employer reporting.
// Default Tinybird datasource: incubar_usage_events.

import { createAuth } from './auth';

interface Env {
  TENANT_KEYS: SimpleKVNamespace;
  AUTH_DB: D1Database;
  AUTH_SECRET: string;
  AUTH_BASE_URL: string;
  AUTH_TRUSTED_ORIGINS?: string;
  TINYBIRD_HOST: string;
  TINYBIRD_DATASOURCE: string;
  TINYBIRD_APPEND_TOKEN: string;
}

interface SimpleKVNamespace {
  get<T = string>(
    key: string,
    type?: 'text' | 'json' | 'arrayBuffer' | 'stream'
  ): Promise<T | null>;
}

interface TenantKeyRecord {
  tenantId: string;
  active?: boolean;
  companyName?: string;
}

interface GatewayProviderRow {
  providerId: string;
  providerEnabled: boolean;
  providerPlan?: string;
  primaryUsedPercent?: number;
  primaryRemainingPercent?: number;
  primaryWindowMinutes?: number;
  primaryResetsAt?: string;
  secondaryUsedPercent?: number;
  secondaryRemainingPercent?: number;
  secondaryWindowMinutes?: number;
  secondaryResetsAt?: string;
  tertiaryUsedPercent?: number;
  tertiaryRemainingPercent?: number;
  tertiaryWindowMinutes?: number;
  tertiaryResetsAt?: string;
  creditsRemaining?: number;
  creditsTotal?: number;
  creditsUnit?: string;
  creditsRemainingPercent?: number;
  costTodayAmountUsd?: number;
  costTodayTokens?: number;
  costMonthAmountUsd?: number;
  costMonthTokens?: number;
  costCurrency?: string;
  snapshotUpdatedAt?: string;
  fetchError?: string;
}

interface GatewayReportPayload {
  schemaVersion: number;
  reportId: string;
  reportTs: string;
  sendReason: string;
  appInstallIdHash: string;
  appVersion: string;
  platform: string;
  employeeId: string;
  employeeEmail?: string;
  enabledProviders: string[];
  enabledProviderCount: number;
  rows: GatewayProviderRow[];
}

type TinybirdRow = Record<string, unknown>;

const MAX_ROWS_PER_REPORT = 64;
const MAX_STRING_LENGTH = 512;
const MAX_DASHBOARD_DAYS = 365;
const MAX_DASHBOARD_ROWS = 200;
const CORS_HEADERS = {
  'access-control-allow-origin': '*',
  'access-control-allow-methods': 'GET,POST,OPTIONS',
  'access-control-allow-headers': 'authorization,content-type,x-api-key,x-organization-id',
  'access-control-expose-headers': 'set-auth-token',
};
const TINYBIRD_COLUMNS = [
  'event_id',
  'report_id',
  'report_ts',
  'send_reason',
  'schema_version',
  'tenant_id',
  'employee_id',
  'employee_email',
  'app_install_id_hash',
  'app_version',
  'platform',
  'provider_id',
  'provider_enabled',
  'provider_plan',
  'primary_used_percent',
  'primary_remaining_percent',
  'primary_window_minutes',
  'primary_resets_at',
  'secondary_used_percent',
  'secondary_remaining_percent',
  'secondary_window_minutes',
  'secondary_resets_at',
  'tertiary_used_percent',
  'tertiary_remaining_percent',
  'tertiary_window_minutes',
  'tertiary_resets_at',
  'credits_remaining',
  'credits_total',
  'credits_unit',
  'credits_remaining_percent',
  'cost_today_amount_usd',
  'cost_today_tokens',
  'cost_month_amount_usd',
  'cost_month_tokens',
  'cost_currency',
  'snapshot_updated_at',
  'fetch_error',
  'enabled_providers',
  'enabled_provider_count',
] as const;

const jsonResponse = (body: unknown, status = 200, extraHeaders?: Record<string, string>): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      'content-type': 'application/json; charset=utf-8',
      'cache-control': 'no-store',
      ...CORS_HEADERS,
      ...(extraHeaders ?? {}),
    },
  });

const emptyResponse = (status = 204): Response =>
  new Response(null, {
    status,
    headers: {
      ...CORS_HEADERS,
    },
  });

const normalizePath = (pathname: string): string => {
  if (pathname === '/') {
    return pathname;
  }
  return pathname.replace(/\/+$/, '');
};

const isPlainObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const asTrimmedString = (value: unknown, maxLen = MAX_STRING_LENGTH): string | undefined => {
  if (typeof value !== 'string') {
    return undefined;
  }
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }
  if (trimmed.length > maxLen) {
    return trimmed.slice(0, maxLen);
  }
  return trimmed;
};

const asNumber = (value: unknown): number | undefined => {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return undefined;
  }
  return value;
};

const asBool = (value: unknown): boolean => value === true;

const toTinybirdDateTime = (value: string | undefined): string | null => {
  if (!value) {
    return null;
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }

  return parsed.toISOString().replace('T', ' ').replace('Z', '').slice(0, 23);
};

const toClickHouseArrayLiteral = (value: unknown): string => {
  if (!Array.isArray(value)) {
    return '[]';
  }

  const normalized = value
    .filter((entry) => typeof entry === 'string')
    .map((entry) => {
      const escaped = entry.replace(/\\/g, '\\\\').replace(/'/g, "\\'");
      return `'${escaped}'`;
    });

  return `[${normalized.join(',')}]`;
};

const toCsvField = (value: unknown): string => {
  if (value === null || value === undefined) {
    return '';
  }

  const serialized =
    Array.isArray(value)
      ? toClickHouseArrayLiteral(value)
      : typeof value === 'number'
      ? Number.isFinite(value)
        ? `${value}`
        : ''
      : `${value}`;

  const escaped = serialized.replace(/"/g, '""');
  if (escaped.includes(',') || escaped.includes('\n') || escaped.includes('\r') || escaped.includes('"')) {
    return `"${escaped}"`;
  }
  return escaped;
};

const toCsvPayload = (rows: TinybirdRow[]): string => {
  const header = TINYBIRD_COLUMNS.join(',');
  const dataLines = rows.map((row) =>
    TINYBIRD_COLUMNS.map((column) => toCsvField(row[column])).join(',')
  );
  return `${header}\n${dataLines.join('\n')}\n`;
};

const parseBoundedInteger = (
  rawValue: string | null,
  fallback: number,
  minimum: number,
  maximum: number
): number => {
  if (!rawValue) {
    return fallback;
  }
  const parsed = Number(rawValue);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  const normalized = Math.trunc(parsed);
  if (normalized < minimum) {
    return minimum;
  }
  if (normalized > maximum) {
    return maximum;
  }
  return normalized;
};

const escapeSqlString = (value: string): string => value.replace(/'/g, "''");

const parseGatewayRow = (value: unknown): GatewayProviderRow | null => {
  if (!isPlainObject(value)) {
    return null;
  }

  const providerId = asTrimmedString(value.providerId);
  if (!providerId) {
    return null;
  }

  return {
    providerId,
    providerEnabled: asBool(value.providerEnabled),
    providerPlan: asTrimmedString(value.providerPlan),
    primaryUsedPercent: asNumber(value.primaryUsedPercent),
    primaryRemainingPercent: asNumber(value.primaryRemainingPercent),
    primaryWindowMinutes: asNumber(value.primaryWindowMinutes),
    primaryResetsAt: asTrimmedString(value.primaryResetsAt),
    secondaryUsedPercent: asNumber(value.secondaryUsedPercent),
    secondaryRemainingPercent: asNumber(value.secondaryRemainingPercent),
    secondaryWindowMinutes: asNumber(value.secondaryWindowMinutes),
    secondaryResetsAt: asTrimmedString(value.secondaryResetsAt),
    tertiaryUsedPercent: asNumber(value.tertiaryUsedPercent),
    tertiaryRemainingPercent: asNumber(value.tertiaryRemainingPercent),
    tertiaryWindowMinutes: asNumber(value.tertiaryWindowMinutes),
    tertiaryResetsAt: asTrimmedString(value.tertiaryResetsAt),
    creditsRemaining: asNumber(value.creditsRemaining),
    creditsTotal: asNumber(value.creditsTotal),
    creditsUnit: asTrimmedString(value.creditsUnit),
    creditsRemainingPercent: asNumber(value.creditsRemainingPercent),
    costTodayAmountUsd: asNumber(value.costTodayAmountUsd),
    costTodayTokens: asNumber(value.costTodayTokens),
    costMonthAmountUsd: asNumber(value.costMonthAmountUsd),
    costMonthTokens: asNumber(value.costMonthTokens),
    costCurrency: asTrimmedString(value.costCurrency),
    snapshotUpdatedAt: asTrimmedString(value.snapshotUpdatedAt),
    fetchError: asTrimmedString(value.fetchError, 4096),
  };
};

const parsePayload = (value: unknown): GatewayReportPayload | null => {
  if (!isPlainObject(value)) {
    return null;
  }

  const reportId = asTrimmedString(value.reportId);
  const reportTs = asTrimmedString(value.reportTs);
  const sendReason = asTrimmedString(value.sendReason);
  const appInstallIdHash = asTrimmedString(value.appInstallIdHash);
  const appVersion = asTrimmedString(value.appVersion);
  const platform = asTrimmedString(value.platform);
  const employeeId = asTrimmedString(value.employeeId, 128);

  if (
    reportId === undefined ||
    reportTs === undefined ||
    sendReason === undefined ||
    appInstallIdHash === undefined ||
    appVersion === undefined ||
    platform === undefined ||
    employeeId === undefined
  ) {
    return null;
  }

  if (typeof value.schemaVersion !== 'number' || !Number.isFinite(value.schemaVersion)) {
    return null;
  }

  if (!Array.isArray(value.enabledProviders) || !Array.isArray(value.rows)) {
    return null;
  }

  const enabledProviders = value.enabledProviders
    .map((entry) => asTrimmedString(entry))
    .filter((entry): entry is string => entry !== undefined);

  const rows = value.rows
    .map((entry) => parseGatewayRow(entry))
    .filter((entry): entry is GatewayProviderRow => entry !== null);

  if (rows.length === 0 || rows.length > MAX_ROWS_PER_REPORT) {
    return null;
  }

  const enabledProviderCount =
    typeof value.enabledProviderCount === 'number' && Number.isFinite(value.enabledProviderCount)
      ? Math.max(0, Math.min(255, Math.trunc(value.enabledProviderCount)))
      : enabledProviders.length;

  return {
    schemaVersion: Math.trunc(value.schemaVersion),
    reportId,
    reportTs,
    sendReason,
    appInstallIdHash,
    appVersion,
    platform,
    employeeId,
    employeeEmail: asTrimmedString(value.employeeEmail),
    enabledProviders,
    enabledProviderCount,
    rows,
  };
};

const toHex = (buffer: ArrayBuffer): string => {
  const bytes = new Uint8Array(buffer);
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
};

const sha256Hex = async (value: string): Promise<string> => {
  const input = new TextEncoder().encode(value);
  const digest = await crypto.subtle.digest('SHA-256', input);
  return toHex(digest);
};

const withCors = (response: Response): Response => {
  const headers = new Headers(response.headers);
  Object.entries(CORS_HEADERS).forEach(([name, value]) => {
    headers.set(name, value);
  });

  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
};

const toValidTenantId = (value: unknown): string | null => {
  const tenantId = asTrimmedString(value, 128);
  if (!tenantId) {
    return null;
  }
  if (!/^[A-Za-z0-9_-]{1,128}$/.test(tenantId)) {
    return null;
  }
  return tenantId;
};

const recordOrNull = (value: unknown): Record<string, unknown> | null =>
  isPlainObject(value) ? value : null;

const extractApiKey = (request: Request): string | null => {
  const direct = asTrimmedString(request.headers.get('x-api-key'));
  if (direct) {
    return direct;
  }

  const authHeader = request.headers.get('authorization');
  if (!authHeader || !authHeader.startsWith('Bearer ')) {
    return null;
  }

  const raw = authHeader.slice('Bearer '.length).trim();
  return raw.length > 0 ? raw : null;
};

const extractTenantIdFromApiKeyVerification = (payload: unknown): string | null => {
  const root = recordOrNull(payload);
  if (!root) {
    return null;
  }

  const keyObject =
    recordOrNull(root.key) ??
    recordOrNull(root.data) ??
    recordOrNull(recordOrNull(root.data)?.key) ??
    root;

  const metadata =
    recordOrNull(keyObject.metadata) ??
    recordOrNull(recordOrNull(keyObject.data)?.metadata) ??
    null;

  const tenantFromMetadata = metadata ? toValidTenantId(metadata.tenantId) : null;
  if (tenantFromMetadata) {
    return tenantFromMetadata;
  }

  return null;
};

const extractActiveOrganizationId = (payload: unknown): string | null => {
  const root = recordOrNull(payload);
  if (!root) {
    return null;
  }

  const sessionObject = recordOrNull(root.session) ?? recordOrNull(recordOrNull(root.data)?.session) ?? root;

  return (
    toValidTenantId(sessionObject.activeOrganizationId) ??
    toValidTenantId(recordOrNull(sessionObject.activeOrganization)?.id) ??
    null
  );
};

const extractOrganizationIds = (payload: unknown): string[] => {
  const records: Record<string, unknown>[] = [];

  if (Array.isArray(payload)) {
    payload.forEach((entry) => {
      const record = recordOrNull(entry);
      if (record) {
        records.push(record);
      }
    });
  } else {
    const root = recordOrNull(payload);
    if (!root) {
      return [];
    }

    const sourceList =
      (Array.isArray(root.data) ? root.data : null) ??
      (Array.isArray(root.organizations) ? root.organizations : null) ??
      (Array.isArray(recordOrNull(root.data)?.organizations)
        ? (recordOrNull(root.data)?.organizations as unknown[])
        : null);

    if (!sourceList) {
      return [];
    }

    sourceList.forEach((entry) => {
      const record = recordOrNull(entry);
      if (record) {
        records.push(record);
      }
    });
  }

  const ids = new Set<string>();
  records.forEach((entry) => {
    const id = toValidTenantId(entry.id) ?? toValidTenantId(entry.organizationId);
    if (id) {
      ids.add(id);
    }
  });

  return [...ids];
};

interface OrganizationSummary {
  id: string;
  name?: string;
  slug?: string;
}

const extractOrganizationSummaries = (payload: unknown): OrganizationSummary[] => {
  const root = recordOrNull(payload);
  const sourceList =
    (Array.isArray(payload) ? payload : null) ??
    (root && Array.isArray(root.data) ? root.data : null) ??
    (root && Array.isArray(root.organizations) ? root.organizations : null) ??
    (root && Array.isArray(recordOrNull(root.data)?.organizations)
      ? (recordOrNull(root.data)?.organizations as unknown[])
      : null);

  if (!sourceList) {
    return [];
  }

  const summaries: OrganizationSummary[] = [];
  sourceList.forEach((entry) => {
    const record = recordOrNull(entry);
    if (!record) {
      return;
    }

    const id = toValidTenantId(record.id) ?? toValidTenantId(record.organizationId);
    if (!id) {
      return;
    }

    summaries.push({
      id,
      name: asTrimmedString(record.name, 256),
      slug: asTrimmedString(record.slug, 128),
    });
  });

  return summaries;
};

const resolveTenantFromAuth = async (request: Request, env: Env): Promise<TenantKeyRecord | null> => {
  const auth = createAuth(env);
  const authApi = auth.api as Record<string, ((input: unknown) => Promise<unknown>) | undefined>;
  const headers = new Headers(request.headers);
  const apiKey = extractApiKey(request);
  if (apiKey && !headers.has('x-api-key')) {
    headers.set('x-api-key', apiKey);
  }

  if (apiKey && typeof authApi.verifyApiKey === 'function') {
    try {
      const verification = await authApi.verifyApiKey({
        body: {
          key: apiKey,
        },
      });
      const tenantId = extractTenantIdFromApiKeyVerification(verification);
      if (tenantId) {
        return {
          tenantId,
          active: true,
        };
      }
    } catch {
      // Continue with session auth fallback.
    }
  }

  let sessionPayload: unknown = null;
  try {
    if (typeof authApi.getSession !== 'function') {
      return null;
    }

    sessionPayload = await authApi.getSession({
      headers,
    });
  } catch {
    return null;
  }

  if (!sessionPayload) {
    return null;
  }

  const requestedOrganizationId = toValidTenantId(request.headers.get('x-organization-id'));

  if (requestedOrganizationId && typeof authApi.listOrganizations === 'function') {
    try {
      const organizationsPayload = await authApi.listOrganizations({
        headers,
      });
      const organizationIds = extractOrganizationIds(organizationsPayload);
      if (organizationIds.includes(requestedOrganizationId)) {
        return {
          tenantId: requestedOrganizationId,
          active: true,
        };
      }
      return null;
    } catch {
      return null;
    }
  }

  const activeOrganizationId = extractActiveOrganizationId(sessionPayload);
  if (activeOrganizationId) {
    return {
      tenantId: activeOrganizationId,
      active: true,
    };
  }

  try {
    if (typeof authApi.listOrganizations !== 'function') {
      return null;
    }

    const organizationsPayload = await authApi.listOrganizations({
      headers,
    });
    const organizationIds = extractOrganizationIds(organizationsPayload);
    if (organizationIds.length === 1) {
      return {
        tenantId: organizationIds[0],
        active: true,
      };
    }
  } catch {
    // Session exists but no list access. Fall through.
  }

  return null;
};

const resolveTenantFromLegacyKey = async (request: Request, env: Env): Promise<TenantKeyRecord | null> => {
  const key = extractApiKey(request);
  if (!key) {
    return null;
  }

  const keyHash = await sha256Hex(key);
  const record = await env.TENANT_KEYS.get<TenantKeyRecord>(`key:${keyHash}`, 'json');
  if (!record || !record.tenantId || record.active === false) {
    return null;
  }

  if (!/^[A-Za-z0-9_-]{1,128}$/.test(record.tenantId)) {
    return null;
  }

  return record;
};

const resolveTenant = async (request: Request, env: Env): Promise<TenantKeyRecord | null> => {
  const authTenant = await resolveTenantFromAuth(request, env);
  if (authTenant) {
    return authTenant;
  }

  return resolveTenantFromLegacyKey(request, env);
};

const runTinybirdSql = async (
  env: Env,
  sql: string,
  tenantId: string
): Promise<Array<Record<string, unknown>>> => {
  const tinybirdHost = env.TINYBIRD_HOST.replace(/\/+$/, '');
  const query = `${sql}\nFORMAT JSON`;
  const url = `${tinybirdHost}/v0/sql?${new URLSearchParams({ q: query }).toString()}`;

  const response = await fetch(url, {
    method: 'GET',
    headers: {
      authorization: `Bearer ${env.TINYBIRD_APPEND_TOKEN}`,
    },
  });

  if (!response.ok) {
    const upstream = await response.text();
    throw new Error(`Tinybird SQL failed for tenant ${tenantId}: ${response.status} ${upstream}`);
  }

  const payload = (await response.json()) as unknown;
  if (!isPlainObject(payload) || !Array.isArray(payload.data)) {
    throw new Error(`Tinybird SQL returned invalid JSON for tenant ${tenantId}`);
  }

  return payload.data.filter(isPlainObject);
};

const dashboardSummarySql = (tenantId: string): string => {
  const tenant = escapeSqlString(tenantId);
  return `
SELECT
  countDistinct(employee_id) AS employees,
  countDistinctIf(provider_id, provider_enabled = 1) AS active_providers,
  round(avgIf(primary_remaining_percent, provider_enabled = 1), 2) AS avg_primary_remaining_percent,
  round(sumIf(cost_today_amount_usd, provider_enabled = 1), 2) AS total_cost_today_amount_usd,
  round(sumIf(cost_month_amount_usd, provider_enabled = 1), 2) AS total_cost_month_amount_usd,
  countIf(fetch_error IS NOT NULL AND fetch_error != '') AS providers_with_errors
FROM incubar_employee_provider_latest
WHERE tenant_id = '${tenant}'`;
};

const dashboardDailySql = (tenantId: string, days: number): string => {
  const tenant = escapeSqlString(tenantId);
  return `
SELECT
  day,
  sum(active_employee_provider_pairs) AS active_employee_provider_pairs,
  round(avgOrNull(avg_primary_remaining_percent), 2) AS avg_primary_remaining_percent,
  round(sumOrNull(total_cost_today_amount_usd), 2) AS total_cost_today_amount_usd,
  sum(provider_errors) AS provider_errors
FROM incubar_provider_daily
WHERE tenant_id = '${tenant}'
  AND day >= today() - ${days}
GROUP BY day
ORDER BY day ASC`;
};

const dashboardProvidersSql = (tenantId: string, limit: number): string => {
  const tenant = escapeSqlString(tenantId);
  return `
SELECT
  provider_id,
  countDistinctIf(employee_id, provider_enabled = 1) AS employees,
  round(avgIf(primary_remaining_percent, provider_enabled = 1), 2) AS avg_primary_remaining_percent,
  round(sumIf(cost_today_amount_usd, provider_enabled = 1), 2) AS total_cost_today_amount_usd,
  round(sumIf(cost_month_amount_usd, provider_enabled = 1), 2) AS total_cost_month_amount_usd,
  countIf(fetch_error IS NOT NULL AND fetch_error != '') AS provider_errors
FROM incubar_employee_provider_latest
WHERE tenant_id = '${tenant}'
GROUP BY provider_id
ORDER BY total_cost_month_amount_usd DESC, employees DESC, provider_id ASC
LIMIT ${limit}`;
};

const dashboardEmployeesSql = (tenantId: string, limit: number): string => {
  const tenant = escapeSqlString(tenantId);
  return `
SELECT
  employee_id,
  countIf(provider_enabled = 1) AS enabled_providers,
  round(avgIf(primary_remaining_percent, provider_enabled = 1), 2) AS avg_primary_remaining_percent,
  round(minIf(primary_remaining_percent, provider_enabled = 1), 2) AS min_primary_remaining_percent,
  round(sumIf(cost_today_amount_usd, provider_enabled = 1), 2) AS total_cost_today_amount_usd,
  round(sumIf(cost_month_amount_usd, provider_enabled = 1), 2) AS total_cost_month_amount_usd,
  countIf(fetch_error IS NOT NULL AND fetch_error != '') AS provider_errors,
  max(last_report_ts) AS last_report_ts
FROM incubar_employee_provider_latest
WHERE tenant_id = '${tenant}'
GROUP BY employee_id
ORDER BY total_cost_month_amount_usd DESC, min_primary_remaining_percent ASC, employee_id ASC
LIMIT ${limit}`;
};

const toTinybirdRows = (payload: GatewayReportPayload, tenant: TenantKeyRecord): TinybirdRow[] => {
  return payload.rows.map((row, index) => ({
    event_id: `${payload.reportId}:${row.providerId}:${index}`,
    report_id: payload.reportId,
    report_ts: toTinybirdDateTime(payload.reportTs),
    send_reason: payload.sendReason,
    schema_version: payload.schemaVersion,

    tenant_id: tenant.tenantId,
    employee_id: payload.employeeId,
    employee_email: payload.employeeEmail ?? null,
    app_install_id_hash: payload.appInstallIdHash,
    app_version: payload.appVersion,
    platform: payload.platform,

    provider_id: row.providerId,
    provider_enabled: row.providerEnabled ? 1 : 0,
    provider_plan: row.providerPlan ?? null,

    primary_used_percent: row.primaryUsedPercent ?? null,
    primary_remaining_percent: row.primaryRemainingPercent ?? null,
    primary_window_minutes: row.primaryWindowMinutes ?? null,
    primary_resets_at: toTinybirdDateTime(row.primaryResetsAt),

    secondary_used_percent: row.secondaryUsedPercent ?? null,
    secondary_remaining_percent: row.secondaryRemainingPercent ?? null,
    secondary_window_minutes: row.secondaryWindowMinutes ?? null,
    secondary_resets_at: toTinybirdDateTime(row.secondaryResetsAt),

    tertiary_used_percent: row.tertiaryUsedPercent ?? null,
    tertiary_remaining_percent: row.tertiaryRemainingPercent ?? null,
    tertiary_window_minutes: row.tertiaryWindowMinutes ?? null,
    tertiary_resets_at: toTinybirdDateTime(row.tertiaryResetsAt),

    credits_remaining: row.creditsRemaining ?? null,
    credits_total: row.creditsTotal ?? null,
    credits_unit: row.creditsUnit ?? null,
    credits_remaining_percent: row.creditsRemainingPercent ?? null,

    cost_today_amount_usd: row.costTodayAmountUsd ?? null,
    cost_today_tokens: row.costTodayTokens ?? null,
    cost_month_amount_usd: row.costMonthAmountUsd ?? null,
    cost_month_tokens: row.costMonthTokens ?? null,
    cost_currency: row.costCurrency ?? null,

    snapshot_updated_at: toTinybirdDateTime(row.snapshotUpdatedAt),
    fetch_error: row.fetchError ?? null,

    enabled_providers: payload.enabledProviders,
    enabled_provider_count: payload.enabledProviderCount,
  }));
};

const postToTinybird = async (env: Env, rows: TinybirdRow[]): Promise<Response> => {
  const tinybirdHost = env.TINYBIRD_HOST.replace(/\/+$/, '');
  const datasourceName = env.TINYBIRD_DATASOURCE;
  const url = `${tinybirdHost}/v0/datasources?name=${encodeURIComponent(datasourceName)}&mode=append&format=csv&skip_first_lines=1`;
  const csvBody = toCsvPayload(rows);

  return fetch(url, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${env.TINYBIRD_APPEND_TOKEN}`,
      'content-type': 'text/csv',
    },
    body: csvBody,
  });
};

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const path = normalizePath(url.pathname);

    if (request.method === 'OPTIONS') {
      return emptyResponse();
    }

    if (request.method === 'GET' && path === '/health') {
      return jsonResponse({ ok: true });
    }

    if (path.startsWith('/api/auth')) {
      const auth = createAuth(env);
      const response = await auth.handler(request);
      return withCors(response);
    }

    if (request.method === 'GET' && path === '/v1/dashboard/organizations') {
      const auth = createAuth(env);
      const authApi = auth.api as Record<string, ((input: unknown) => Promise<unknown>) | undefined>;
      if (typeof authApi.getSession !== 'function' || typeof authApi.listOrganizations !== 'function') {
        return jsonResponse({ error: 'Auth API not available' }, 501);
      }

      const headers = new Headers(request.headers);
      const apiKey = extractApiKey(request);
      if (apiKey && !headers.has('x-api-key')) {
        headers.set('x-api-key', apiKey);
      }

      try {
        const sessionPayload = await authApi.getSession({ headers });
        if (!sessionPayload) {
          return jsonResponse({ error: 'Unauthorized' }, 401);
        }

        const organizationsPayload = await authApi.listOrganizations({ headers });
        const organizations = extractOrganizationSummaries(organizationsPayload);

        return jsonResponse({
          ok: true,
          activeOrganizationId: extractActiveOrganizationId(sessionPayload),
          organizations,
        });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return jsonResponse({ error: `Could not load organizations: ${message}` }, 502);
      }
    }

    if (path.startsWith('/v1/dashboard/')) {
      if (request.method !== 'GET') {
        return jsonResponse({ error: 'Method not allowed' }, 405);
      }

      const tenant = await resolveTenant(request, env);
      if (!tenant) {
        return jsonResponse({ error: 'Unauthorized' }, 401);
      }

      const days = parseBoundedInteger(url.searchParams.get('days'), 30, 1, MAX_DASHBOARD_DAYS);
      const limit = parseBoundedInteger(url.searchParams.get('limit'), 25, 1, MAX_DASHBOARD_ROWS);

      try {
        if (path === '/v1/dashboard/summary') {
          const rows = await runTinybirdSql(env, dashboardSummarySql(tenant.tenantId), tenant.tenantId);
          return jsonResponse({
            ok: true,
            tenantId: tenant.tenantId,
            data: rows[0] ?? {},
          });
        }

        if (path === '/v1/dashboard/daily') {
          const rows = await runTinybirdSql(env, dashboardDailySql(tenant.tenantId, days), tenant.tenantId);
          return jsonResponse({
            ok: true,
            tenantId: tenant.tenantId,
            days,
            data: rows,
          });
        }

        if (path === '/v1/dashboard/providers') {
          const rows = await runTinybirdSql(
            env,
            dashboardProvidersSql(tenant.tenantId, limit),
            tenant.tenantId
          );
          return jsonResponse({
            ok: true,
            tenantId: tenant.tenantId,
            limit,
            data: rows,
          });
        }

        if (path === '/v1/dashboard/employees') {
          const rows = await runTinybirdSql(
            env,
            dashboardEmployeesSql(tenant.tenantId, limit),
            tenant.tenantId
          );
          return jsonResponse({
            ok: true,
            tenantId: tenant.tenantId,
            limit,
            data: rows,
          });
        }

        if (path === '/v1/dashboard/overview') {
          const [summaryRows, dailyRows, providerRows, employeeRows] = await Promise.all([
            runTinybirdSql(env, dashboardSummarySql(tenant.tenantId), tenant.tenantId),
            runTinybirdSql(env, dashboardDailySql(tenant.tenantId, days), tenant.tenantId),
            runTinybirdSql(env, dashboardProvidersSql(tenant.tenantId, limit), tenant.tenantId),
            runTinybirdSql(env, dashboardEmployeesSql(tenant.tenantId, limit), tenant.tenantId),
          ]);

          return jsonResponse({
            ok: true,
            tenantId: tenant.tenantId,
            days,
            limit,
            data: {
              summary: summaryRows[0] ?? {},
              daily: dailyRows,
              providers: providerRows,
              employees: employeeRows,
            },
          });
        }

        return jsonResponse({ error: 'Not found' }, 404);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return jsonResponse({ error: `Dashboard query failed: ${message}` }, 502);
      }
    }

    if (request.method !== 'POST' || path !== '/v1/usage-report') {
      return jsonResponse({ error: 'Not found' }, 404);
    }

    const tenant = await resolveTenant(request, env);
    if (!tenant) {
      return jsonResponse({ error: 'Unauthorized' }, 401);
    }

    let parsedBody: unknown;
    try {
      parsedBody = await request.json();
    } catch {
      return jsonResponse({ error: 'Invalid JSON body' }, 400);
    }

    const payload = parsePayload(parsedBody);
    if (!payload) {
      return jsonResponse({ error: 'Invalid usage report payload' }, 400);
    }

    const rows = toTinybirdRows(payload, tenant);

    let tinybirdResponse: Response;
    try {
      tinybirdResponse = await postToTinybird(env, rows);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return jsonResponse({ error: `Tinybird request failed: ${message}` }, 503);
    }

    if (!tinybirdResponse.ok) {
      const upstreamBody = await tinybirdResponse.text();
      const isRetryable = tinybirdResponse.status === 429 || tinybirdResponse.status >= 500;
      return jsonResponse(
        {
          error: 'Tinybird rejected events',
          upstreamStatus: tinybirdResponse.status,
          upstreamBody,
        },
        isRetryable ? 503 : 400
      );
    }

    return jsonResponse({
      ok: true,
      accepted: rows.length,
      tenantId: tenant.tenantId,
      reportId: payload.reportId,
    });
  },
};
