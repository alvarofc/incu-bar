import { betterAuth } from 'better-auth';
import Database from 'better-sqlite3';
import { apiKey, bearer, organization } from 'better-auth/plugins';

export const auth = betterAuth({
  secret: 'local-cli-generation-secret',
  baseURL: 'http://localhost:8787',
  database: new Database('./auth-cli.sqlite'),
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
