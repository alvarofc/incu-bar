import { z } from 'zod';

const providerIdSchema = z.enum([
  'claude',
  'codex',
  'cursor',
  'copilot',
  'gemini',
  'antigravity',
  'factory',
  'zai',
  'minimax',
  'kimi',
  'kimi_k2',
  'kiro',
  'vertexai',
  'augment',
  'amp',
  'jetbrains',
  'opencode',
  'synthetic',
]);

const rateWindowSchema = z.object({
  usedPercent: z.number(),
  windowMinutes: z.number().optional(),
  resetsAt: z.string().optional(),
  resetDescription: z.string().optional(),
  label: z.string().optional(),
});

const costSnapshotSchema = z.object({
  todayAmount: z.number(),
  todayTokens: z.number(),
  monthAmount: z.number(),
  monthTokens: z.number(),
  currency: z.string(),
});

const providerIdentitySchema = z.object({
  email: z.string().optional(),
  name: z.string().optional(),
  plan: z.string().optional(),
  organization: z.string().optional(),
});

const usageSnapshotSchema = z.object({
  primary: rateWindowSchema.optional(),
  secondary: rateWindowSchema.optional(),
  tertiary: rateWindowSchema.optional(),
  credits: z
    .object({
      remaining: z.number(),
      total: z.number().optional(),
      unit: z.string(),
    })
    .optional(),
  cost: costSnapshotSchema.optional(),
  identity: providerIdentitySchema.optional(),
  updatedAt: z.string(),
  error: z.string().optional(),
});

const usageUpdateEventSchema = z.object({
  providerId: providerIdSchema,
  usage: usageSnapshotSchema,
});

export type UsageUpdatePayload = z.infer<typeof usageUpdateEventSchema>;

export const parseUsageUpdateEvent = (payload: unknown): UsageUpdatePayload | null => {
  const result = usageUpdateEventSchema.safeParse(payload);
  if (!result.success) {
    console.warn('Invalid usage update payload received', result.error);
    return null;
  }
  return result.data;
};
