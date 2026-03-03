import { SignJWT, jwtVerify } from 'jose';
import { cookies } from 'next/headers';

const SECRET = new TextEncoder().encode(process.env.AUTH_SECRET || 'fallback-secret');
const COOKIE_NAME = 'polybot-session';
const TRUE_ENV_VALUES = new Set(['1', 'true', 'yes', 'on']);
const FALSE_ENV_VALUES = new Set(['0', 'false', 'no', 'off']);

export async function createSession(): Promise<string> {
  const token = await new SignJWT({ authenticated: true })
    .setProtectedHeader({ alg: 'HS256' })
    .setIssuedAt()
    .setExpirationTime('24h')
    .sign(SECRET);
  return token;
}

export async function verifySession(token: string): Promise<boolean> {
  try {
    await jwtVerify(token, SECRET);
    return true;
  } catch {
    return false;
  }
}

export async function getSession(): Promise<boolean> {
  const cookieStore = await cookies();
  const token = cookieStore.get(COOKIE_NAME)?.value;
  if (!token) return false;
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
