import { NextRequest, NextResponse } from 'next/server';
import { getSessionCookieName, verifySession } from '@/lib/auth';

const LOGIN_PATH = '/login';
const AUTH_API_PATH = '/api/auth';
const INTERNAL_CLAIM_ADAPTER_PATHS = new Set([
  '/api/internal/claim/redeem',
  '/api/internal/claim/activate-funds',
]);

export async function middleware(req: NextRequest) {
  const { pathname } = req.nextUrl;

  if (pathname.startsWith('/_next') || pathname.startsWith('/favicon')) {
    return NextResponse.next();
  }

  if (isAuthApiPath(pathname)) {
    return NextResponse.next();
  }
  if (INTERNAL_CLAIM_ADAPTER_PATHS.has(pathname)) {
    return NextResponse.next();
  }

  const token = req.cookies.get(getSessionCookieName())?.value;
  const session = token ? await verifySession(token) : null;

  if (isLoginPath(pathname)) {
    if (session) {
      return NextResponse.redirect(new URL('/', req.url));
    }
    return NextResponse.next();
  }

  if (session) {
    return NextResponse.next();
  }
  return redirectToLogin(req);
}

function isLoginPath(pathname: string) {
  return pathname === LOGIN_PATH || pathname.startsWith(`${LOGIN_PATH}/`);
}

function isAuthApiPath(pathname: string) {
  return pathname === AUTH_API_PATH || pathname.startsWith(`${AUTH_API_PATH}/`);
}

function redirectToLogin(req: NextRequest) {
  if (req.nextUrl.pathname.startsWith('/api/')) {
    return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
  }
  const loginUrl = new URL('/login', req.url);
  return NextResponse.redirect(loginUrl);
}

export const config = {
  matcher: ['/((?!_next/static|_next/image|favicon.ico).*)'],
};
