import { betterAuth } from 'better-auth/minimal';
import { apiKey, bearer, organization } from 'better-auth/plugins';
import { createD1Adapter } from './d1-adapter';

export interface BetterAuthEnv {
  AUTH_DB: D1Database;
  AUTH_SECRET: string;
  AUTH_BASE_URL: string;
  AUTH_TRUSTED_ORIGINS?: string;
}

type BetterAuthOptions = Parameters<typeof betterAuth>[0];

const parseTrustedOrigins = (rawValue: string | undefined): string[] => {
  if (!rawValue) {
    return [];
  }

  return rawValue
    .split(',')
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
};

export const buildAuthOptions = (env: BetterAuthEnv): BetterAuthOptions => ({
  secret: env.AUTH_SECRET,
  baseURL: env.AUTH_BASE_URL,
  trustedOrigins: [env.AUTH_BASE_URL, ...parseTrustedOrigins(env.AUTH_TRUSTED_ORIGINS)],
  database: createD1Adapter(env.AUTH_DB),
  emailAndPassword: {
    enabled: true,
    requireEmailVerification: false,
    autoSignIn: true,
  },
  plugins: [
    organization(),
    bearer(),
    apiKey({
      enableSessionForAPIKeys: true,
      apiKeyHeaders: ['x-api-key'],
    }),
  ],
});

export const createAuth = (env: BetterAuthEnv) => betterAuth(buildAuthOptions(env));

export const runAuthMigrations = async () => {
  throw new Error('Better Auth migrations are not supported with D1. Use auth-schema.sql with wrangler d1 execute.');
};
