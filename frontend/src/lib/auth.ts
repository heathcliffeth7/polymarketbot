import { SignJWT, jwtVerify } from 'jose';
import { cookies } from 'next/headers';

const AUTH_SECRET_MIN_LENGTH = 32;
const COOKIE_NAME = 'polybot-session';
const TRUE_ENV_VALUES = new Set(['1', 'true', 'yes', 'on']);
const FALSE_ENV_VALUES = new Set(['0', 'false', 'no', 'off']);

export interface SessionUser {
  userId: number;
  username: string;
}

interface SessionPayload {
  authenticated: true;
  userId: number;
  username: string;
}

function getSecret(): Uint8Array {
  const raw = (process.env.AUTH_SECRET || '').trim();
  if (!raw || raw.includes('CHANGE_ME') || raw.length < AUTH_SECRET_MIN_LENGTH) {
    throw new Error(`AUTH_SECRET must be set to a strong value with at least ${AUTH_SECRET_MIN_LENGTH} characters`);
  }
  return new TextEncoder().encode(raw);
}

export async function createSession(user: SessionUser): Promise<string> {
  return new SignJWT({
    authenticated: true,
    userId: user.userId,
    username: user.username,
  } satisfies SessionPayload)
    .setProtectedHeader({ alg: 'HS256' })
    .setIssuedAt()
    .setExpirationTime('24h')
    .sign(getSecret());
}

export async function verifySession(token: string): Promise<SessionUser | null> {
  try {
    const { payload } = await jwtVerify(token, getSecret());
    const userId = Number(payload.userId);
    const username = String(payload.username ?? '').trim();
    if (!Number.isFinite(userId) || userId <= 0 || !username) {
      return null;
    }
    return { userId, username };
  } catch {
    return null;
  }
}

export async function getSessionUser(): Promise<SessionUser | null> {
  const cookieStore = await cookies();
  const token = cookieStore.get(COOKIE_NAME)?.value;
  if (!token) return null;
  return verifySession(token);
}

export function getSessionCookieName(): string {
  return COOKIE_NAME;
}

export function shouldUseSecureAuthCookie(): boolean {
  const raw = process.env.AUTH_COOKIE_SECURE;
  if (raw) {
    const normalized = raw.trim().toLowerCase();
    if (TRUE_ENV_VALUES.has(normalized)) {
      return true;
    }
    if (FALSE_ENV_VALUES.has(normalized)) {
      return false;
    }
  }

  return process.env.NODE_ENV === 'production';
}
