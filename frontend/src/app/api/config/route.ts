import { NextResponse } from 'next/server';
import { listConfigs, readConfig, isAllowedFile, isWritable } from '@/lib/config';

export const dynamic = 'force-dynamic';

export async function GET() {
  try {
    const files = await listConfigs();
    const configs: Record<string, { data: Record<string, unknown>; writable: boolean }> = {};

    for (const file of files) {
      if (isAllowedFile(file)) {
        try {
          const data = await readConfig(file);
          configs[file] = { data, writable: isWritable(file) };
        } catch (err) {
          console.error(`Failed to read config ${file}:`, err);
        }
      }
    }

    return NextResponse.json(configs);
  } catch (err) {
    console.error('Config list error:', err);
    return NextResponse.json({ error: 'Failed to list configs' }, { status: 500 });
  }
}
