# Employer Reporting Gateway (Cloudflare Worker)

This Worker receives usage reports from IncuBar desktop clients, resolves tenant identity, and forwards rows to Tinybird.

The gateway now supports **Better Auth + Organization plugin** for tenant-aware auth and dashboard access.

## Endpoints

- `GET /health`
- `GET|POST /api/auth/*` (Better Auth routes)
- `GET /v1/dashboard/organizations`
- `POST /v1/usage-report`
- `GET /v1/dashboard/overview?days=30&limit=25`
- `GET /v1/dashboard/summary`
- `GET /v1/dashboard/daily?days=30`
- `GET /v1/dashboard/providers?limit=25`
- `GET /v1/dashboard/employees?limit=25`

## Security model

- Better Auth organization session (bearer token) can authorize dashboard reads and map to active org.
- Better Auth API keys can be used for machine access (`x-api-key`) and can carry tenant metadata.
- Legacy tenant key lookup in KV (`TENANT_KEYS`) remains as backward-compatible fallback.
- Tinybird write token stays in Worker secret storage.
- Dashboard routes return tenant-scoped metrics only.

## Required configuration

Set in `wrangler.toml`:

- `TINYBIRD_HOST`
- `TINYBIRD_DATASOURCE`
- `TENANT_KEYS` KV binding
- `AUTH_DB` D1 binding
- `AUTH_BASE_URL`
- `AUTH_TRUSTED_ORIGINS`

Set as secrets:

```bash
wrangler secret put TINYBIRD_APPEND_TOKEN
wrangler secret put AUTH_SECRET
```

## Better Auth migration

Run once after deploy:

```bash
cd infra/cloudflare-gateway
npx @better-auth/cli@latest generate --config auth.cli.ts --output auth-schema.sql --yes
npx wrangler d1 execute incubar-auth --remote --file auth-schema.sql
```

This creates/updates Better Auth tables (including organization plugin tables).

## Tenant key provisioning

For each tenant, generate a random key and store a KV record by hash:

- KV key: `key:<sha256_hex(api_key)>`
- KV value (JSON): `{"tenantId":"acme","active":true}`

## Better Auth organization flow (recommended)

1. Create account: `POST /api/auth/sign-up/email`
2. Sign in: `POST /api/auth/sign-in/email`
3. Create organization: `POST /api/auth/organization/create`
4. (Optional) create API key with tenant metadata (`tenantId`) for desktop ingestion
5. Dashboard reads `GET /v1/dashboard/organizations` and `GET /v1/dashboard/overview`

## Local development

```bash
cd infra/cloudflare-gateway
npm install
npm run dev
```

## Deployment

```bash
cd infra/cloudflare-gateway
npm run deploy
```
