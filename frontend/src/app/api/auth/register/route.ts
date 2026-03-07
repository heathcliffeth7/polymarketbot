import { NextRequest, NextResponse } from 'next/server';
import {
  createSession,
  getSessionCookieName,
  shouldUseSecureAuthCookie,
} from '@/lib/auth';
import { seedUserConfigsFromFiles } from '@/lib/config';
import {
  getAuthStatusPayload,
  registerUser,
} from '@/lib/auth-db';

export async function GET() {
  try {
    const payload = await getAuthStatusPayload(null);
    return NextResponse.json({
      registrationOpen: payload.registrationOpen,
      userCount: payload.userCount,
      maxUsers: payload.maxUsers,
    });
  } catch {
    return NextResponse.json({ error: 'Failed to load registration status' }, { status: 500 });
  }
}

export async function POST(req: NextRequest) {
  try {
    const body = await req.json();
    const username = String(body?.username || '');
    const password = String(body?.password || '');
    const user = await registerUser(username, password);
    await seedUserConfigsFromFiles(user);

    const token = await createSession(user);
    const response = NextResponse.json({ success: true, user }, { status: 201 });
    const secureCookie = shouldUseSecureAuthCookie();

    response.cookies.set(getSessionCookieName(), token, {
      httpOnly: true,
      secure: secureCookie,
      sameSite: 'lax',
      maxAge: 60 * 60 * 24,
      path: '/',
    });

    return response;
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Failed to register';
    const status = message === 'Registration is closed' ? 403 : 400;
    return NextResponse.json({ error: message }, { status });
  }
}
