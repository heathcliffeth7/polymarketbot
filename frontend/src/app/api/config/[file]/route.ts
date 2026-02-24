import { NextRequest, NextResponse } from 'next/server';
import { readConfig, writeConfig, isAllowedFile, isWritable } from '@/lib/config';

export const dynamic = 'force-dynamic';

export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ file: string }> }
) {
  try {
    const { file } = await params;
    if (!isAllowedFile(file)) {
      return NextResponse.json({ error: 'File not allowed' }, { status: 403 });
    }
    const data = await readConfig(file);
    return NextResponse.json({ data, writable: isWritable(file) });
  } catch (err) {
    console.error('Config read error:', err);
    return NextResponse.json({ error: 'Failed to read config' }, { status: 500 });
  }
}

export async function PUT(
  req: NextRequest,
  { params }: { params: Promise<{ file: string }> }
) {
  try {
    const { file } = await params;
    if (!isAllowedFile(file)) {
      return NextResponse.json({ error: 'File not allowed' }, { status: 403 });
    }
    if (!isWritable(file)) {
      return NextResponse.json({ error: 'File is read-only' }, { status: 403 });
    }

    const data = await req.json();
    await writeConfig(file, data);
    return NextResponse.json({ success: true });
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Failed to write config';
    console.error('Config write error:', err);
    return NextResponse.json({ error: message }, { status: 400 });
  }
}
