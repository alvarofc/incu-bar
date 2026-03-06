import { createAdapterFactory } from 'better-auth/adapters';
import type { Where } from 'better-auth/adapters';

const IDENTIFIER_PATTERN = /^[A-Za-z0-9_]+$/;

const quoteIdentifier = (value: string): string => {
  if (!IDENTIFIER_PATTERN.test(value)) {
    throw new Error(`Invalid SQL identifier: ${value}`);
  }
  return `"${value}"`;
};

const buildWhereClause = (where?: Where[]): { clause: string; values: unknown[] } => {
  if (!where || where.length === 0) {
    return { clause: '', values: [] };
  }

  const parts: string[] = [];
  const values: unknown[] = [];

  where.forEach((entry, index) => {
    const connector = index === 0 ? '' : ` ${entry.connector} `;
    const column = quoteIdentifier(entry.field);
    const operator = entry.operator ?? 'eq';

    if ((operator === 'in' || operator === 'not_in') && Array.isArray(entry.value)) {
      if (entry.value.length === 0) {
        const emptyClause = operator === 'in' ? '0=1' : '1=1';
        parts.push(`${connector}(${emptyClause})`);
        return;
      }

      const placeholders = entry.value.map(() => '?').join(',');
      const clause = `${column} ${operator === 'in' ? 'IN' : 'NOT IN'} (${placeholders})`;
      parts.push(`${connector}(${clause})`);
      values.push(...entry.value);
      return;
    }

    if ((operator === 'eq' || operator === 'ne') && entry.value === null) {
      const clause = `${column} IS ${operator === 'eq' ? '' : 'NOT '}NULL`;
      parts.push(`${connector}(${clause})`);
      return;
    }

    let clause = `${column} = ?`;
    let value: unknown = entry.value;

    switch (operator) {
      case 'ne':
        clause = `${column} != ?`;
        break;
      case 'lt':
        clause = `${column} < ?`;
        break;
      case 'lte':
        clause = `${column} <= ?`;
        break;
      case 'gt':
        clause = `${column} > ?`;
        break;
      case 'gte':
        clause = `${column} >= ?`;
        break;
      case 'contains':
        clause = `${column} LIKE ?`;
        value = `%${entry.value ?? ''}%`;
        break;
      case 'starts_with':
        clause = `${column} LIKE ?`;
        value = `${entry.value ?? ''}%`;
        break;
      case 'ends_with':
        clause = `${column} LIKE ?`;
        value = `%${entry.value ?? ''}`;
        break;
      case 'eq':
      default:
        clause = `${column} = ?`;
        break;
    }

    parts.push(`${connector}(${clause})`);
    values.push(value);
  });

  return {
    clause: `WHERE ${parts.join('')}`,
    values,
  };
};

const bindStatement = (db: D1Database, sql: string, values: unknown[]) => {
  const statement = db.prepare(sql);
  return values.length > 0 ? statement.bind(...values) : statement;
};

const toNumber = (value: unknown): number => {
  if (typeof value === 'number') {
    return value;
  }
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) {
      return parsed;
    }
  }
  return 0;
};

export const createD1Adapter = (db: D1Database) => {
  const adapterCreator = createAdapterFactory({
    config: {
      adapterId: 'd1',
      adapterName: 'D1 Adapter',
      usePlural: false,
      debugLogs: false,
      supportsBooleans: false,
      supportsDates: false,
      supportsJSON: false,
      supportsArrays: false,
      supportsUUIDs: false,
      transaction: false,
    },
    adapter: ({ getFieldName }) => ({
      async create<T extends Record<string, any>>({ model, data }: { model: string; data: T }) {
        const entries = Object.entries(data ?? {}).filter(([, value]) => value !== undefined);
        if (entries.length === 0) {
          throw new Error(`Cannot insert empty row for ${model}`);
        }

        const table = quoteIdentifier(model);
        const columns = entries.map(([key]) => quoteIdentifier(key));
        const values = entries.map(([, value]) => value);
        const placeholders = entries.map(() => '?').join(',');
        const sql = `INSERT INTO ${table} (${columns.join(',')}) VALUES (${placeholders})`;

        await bindStatement(db, sql, values).run();

        if ('id' in data) {
          const row = await bindStatement(
            db,
            `SELECT * FROM ${table} WHERE "id" = ? LIMIT 1`,
            [data.id]
          ).all();
          return (row.results?.[0] ?? data) as T;
        }

        return data as T;
      },
      async findOne<T>({ model, where }: { model: string; where: Where[] }) {
        const table = quoteIdentifier(model);
        const { clause, values } = buildWhereClause(where);
        const sql = `SELECT * FROM ${table} ${clause} LIMIT 1`;
        const row = await bindStatement(db, sql, values).all();
        return (row.results?.[0] ?? null) as T | null;
      },
      async findMany<T>({
        model,
        where,
        limit,
        sortBy,
        offset,
      }: {
        model: string;
        where?: Where[];
        limit?: number;
        sortBy?: { field: string; direction: 'asc' | 'desc' };
        offset?: number;
      }) {
        const table = quoteIdentifier(model);
        const { clause, values } = buildWhereClause(where);

        let orderBy = '';
        if (sortBy?.field) {
          const column = quoteIdentifier(
            getFieldName({
              model,
              field: sortBy.field,
            })
          );
          orderBy = ` ORDER BY ${column} ${sortBy.direction === 'desc' ? 'DESC' : 'ASC'}`;
        }

        let pagination = '';
        const paginationValues: unknown[] = [];
        if (typeof limit === 'number') {
          pagination += ' LIMIT ?';
          paginationValues.push(limit);
        }
        if (typeof offset === 'number') {
          pagination += ' OFFSET ?';
          paginationValues.push(offset);
        }

        const sql = `SELECT * FROM ${table} ${clause}${orderBy}${pagination}`;
        const row = await bindStatement(db, sql, [...values, ...paginationValues]).all();
        return (row.results ?? []) as T[];
      },
      async count({ model, where }: { model: string; where?: Where[] }) {
        const table = quoteIdentifier(model);
        const { clause, values } = buildWhereClause(where);
        const sql = `SELECT count(*) as count FROM ${table} ${clause}`;
        const row = await bindStatement(db, sql, values).all();
        return toNumber(row.results?.[0]?.count);
      },
      async update<T>({ model, where, update }: { model: string; where: Where[]; update: T }) {
        const entries = Object.entries(update ?? {}).filter(([, value]) => value !== undefined);
        if (entries.length === 0) {
          return null;
        }

        const table = quoteIdentifier(model);
        const setClauses = entries.map(([key]) => `${quoteIdentifier(key)} = ?`).join(', ');
        const setValues = entries.map(([, value]) => value);
        const { clause, values } = buildWhereClause(where);
        const sql = `UPDATE ${table} SET ${setClauses} ${clause}`;

        await bindStatement(db, sql, [...setValues, ...values]).run();

        const refreshed = await bindStatement(
          db,
          `SELECT * FROM ${table} ${clause} LIMIT 1`,
          values
        ).all();
        return (refreshed.results?.[0] ?? null) as T | null;
      },
      async updateMany({ model, where, update }: { model: string; where: Where[]; update: Record<string, any> }) {
        const entries = Object.entries(update ?? {}).filter(([, value]) => value !== undefined);
        if (entries.length === 0) {
          return 0;
        }

        const table = quoteIdentifier(model);
        const setClauses = entries.map(([key]) => `${quoteIdentifier(key)} = ?`).join(', ');
        const setValues = entries.map(([, value]) => value);
        const { clause, values } = buildWhereClause(where);
        const sql = `UPDATE ${table} SET ${setClauses} ${clause}`;

        const result = await bindStatement(db, sql, [...setValues, ...values]).run();
        return result.meta?.changes ?? 0;
      },
      async delete({ model, where }: { model: string; where: Where[] }) {
        const table = quoteIdentifier(model);
        const { clause, values } = buildWhereClause(where);
        const sql = `DELETE FROM ${table} ${clause}`;
        await bindStatement(db, sql, values).run();
      },
      async deleteMany({ model, where }: { model: string; where: Where[] }) {
        const table = quoteIdentifier(model);
        const { clause, values } = buildWhereClause(where);
        const sql = `DELETE FROM ${table} ${clause}`;
        const result = await bindStatement(db, sql, values).run();
        return result.meta?.changes ?? 0;
      },
    }),
  });

  return (options: Parameters<typeof adapterCreator>[0]) => adapterCreator(options);
};
