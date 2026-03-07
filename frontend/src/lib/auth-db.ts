import { pool } from '@/lib/db';
import type { SessionUser } from '@/lib/auth';

export const MAX_APP_USERS = 2;
const PASSWORD_MIN_LENGTH = 8;
const USERNAME_PATTERN = /^[a-z0-9](?:[a-z0-9._-]{1,30}[a-z0-9])?$/;

export interface AuthStatusPayload {
  authenticated: boolean;
  user: SessionUser | null;
  registrationOpen: boolean;
  userCount: number;
  maxUsers: number;
}

export function normalizeUsername(username: string): string {
  return username.trim().toLowerCase();
}

export function validateUsername(username: string): void {
  const normalized = normalizeUsername(username);
  if (!normalized) {
    throw new Error('Username is required');
  }
  if (normalized.length < 3 || normalized.length > 32) {
    throw new Error('Username must be 3-32 characters');
  }
  if (!USERNAME_PATTERN.test(normalized)) {
    throw new Error('Username may contain only lowercase letters, numbers, dot, dash, underscore');
  }
}

export async function getUserCount(): Promise<number> {
  const res = await pool.query('SELECT COUNT(*)::int AS total FROM app_users');
  return Number(res.rows[0]?.total || 0);
}

export async function isRegistrationOpen(): Promise<boolean> {
  return (await getUserCount()) < MAX_APP_USERS;
}

export async function getAuthStatusPayload(user: SessionUser | null): Promise<AuthStatusPayload> {
  const userCount = await getUserCount();
  return {
    authenticated: !!user,
    user,
    registrationOpen: userCount < MAX_APP_USERS,
    userCount,
    maxUsers: MAX_APP_USERS,
  };
}

export async function authenticateUser(
  username: string,
  password: string
): Promise<SessionUser | null> {
  const normalized = normalizeUsername(username);
  if (!normalized || !password) {
    return null;
  }

  const res = await pool.query(
    `SELECT id, username
     FROM app_users
     WHERE LOWER(username) = LOWER($1)
       AND password_hash = crypt($2, password_hash)
     LIMIT 1`,
    [normalized, password]
  );

  if ((res.rowCount ?? 0) === 0) {
    return null;
  }

  return {
    userId: Number(res.rows[0].id),
    username: String(res.rows[0].username),
  };
}

export async function registerUser(
  username: string,
  password: string
): Promise<SessionUser> {
  const normalized = normalizeUsername(username);
  validateUsername(normalized);
  if (!password || password.length < PASSWORD_MIN_LENGTH) {
    throw new Error(`Password must be at least ${PASSWORD_MIN_LENGTH} characters`);
  }

  const client = await pool.connect();
  try {
    await client.query('BEGIN');
    await client.query('LOCK TABLE app_users IN EXCLUSIVE MODE');

    const countRes = await client.query('SELECT COUNT(*)::int AS total FROM app_users');
    const userCount = Number(countRes.rows[0]?.total || 0);
    if (userCount >= MAX_APP_USERS) {
      throw new Error('Registration is closed');
    }

    const insertRes = await client.query(
      `INSERT INTO app_users (username, password_hash, created_at, updated_at)
       VALUES ($1, crypt($2, gen_salt('bf', 10)), NOW(), NOW())
       RETURNING id, username`,
      [normalized, password]
    );

    await client.query('COMMIT');
    return {
      userId: Number(insertRes.rows[0].id),
      username: String(insertRes.rows[0].username),
    };
  } catch (err) {
    await client.query('ROLLBACK');
    const code =
      err && typeof err === 'object' && 'code' in err ? String(err.code) : '';
    const message = err instanceof Error ? err.message : 'Failed to register user';
    if (
      code === '23505' ||
      message.includes('idx_app_users_username_lower') ||
      message.includes('uq_app_users_username_lower')
    ) {
      throw new Error('Username is already taken');
    }
    if (message.includes('Registration is closed')) {
      throw new Error('Registration is closed');
    }
    throw new Error(message);
  } finally {
    client.release();
  }
}
