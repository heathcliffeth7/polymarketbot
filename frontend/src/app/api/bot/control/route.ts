import { NextRequest, NextResponse } from 'next/server';
import { controlService } from '@/lib/systemctl';

export async function POST(req: NextRequest) {
  try {
    const { action } = await req.json();

    if (!['start', 'stop', 'restart'].includes(action)) {
      return NextResponse.json({ error: 'Invalid action' }, { status: 400 });
    }

    const result = await controlService(action);

    if (!result.controlAvailable) {
      return NextResponse.json(
        {
          error: result.message,
          controlAvailable: false,
          controlReason: result.controlReason,
          controlReasonCode: result.controlReasonCode,
        },
        { status: 503 }
      );
    }

    if (!result.success) {
      return NextResponse.json({ error: result.message }, { status: 500 });
    }

    return NextResponse.json(result);
  } catch (err) {
    console.error('Bot control error:', err);
    return NextResponse.json({ error: 'Failed to control bot' }, { status: 500 });
  }
}
