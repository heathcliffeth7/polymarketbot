import { NextRequest, NextResponse } from 'next/server';
import {
  createSession,
  getSessionCookieName,
  getSessionUser,
  shouldUseSecureAuthCookie,
} from '@/lib/auth';
import {
  authenticateUser,
  getAuthStatusPayload,
} from '@/lib/auth-db';

export async function GET() {
  try {
    const user = await getSessionUser();
    const payload = await getAuthStatusPayload(user);
    return NextResponse.json(payload);
  } catch {
    return NextResponse.json(
      {
        authenticated: false,
        user: null,
        registrationOpen: false,
        userCount: 0,
        maxUsers: 2,
      },
      { status: 500 }
    );
  }
}

export async function POST(req: NextRequest) {
  try {
    const body = await req.json();
    const username = String(body?.username || '');
    const password = String(body?.password || '');
    const user = await authenticateUser(username, password);

    if (!user) {
      return NextResponse.json({ error: 'Invalid username or password' }, { status: 401 });
    }

    const token = await createSession(user);
    const response = NextResponse.json({ success: true, user });
    const secureCookie = shouldUseSecureAuthCookie();

    response.cookies.set(getSessionCookieName(), token, {
      httpOnly: true,
      secure: secureCookie,
      sameSite: 'lax',
      maxAge: 60 * 60 * 24,
      path: '/',
    });

    return response;
  } catch {
    return NextResponse.json({ error: 'Internal error' }, { status: 500 });
  }
}

export async function DELETE() {
  const response = NextResponse.json({ success: true });
  const secureCookie = shouldUseSecureAuthCookie();
  response.cookies.set(getSessionCookieName(), '', {
    httpOnly: true,
    secure: secureCookie,
    sameSite: 'lax',
    maxAge: 0,
    path: '/',
  });
  return response;
}
