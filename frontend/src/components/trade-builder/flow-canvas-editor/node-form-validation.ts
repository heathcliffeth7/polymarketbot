import type {
  EntryTimingProfileRow,
  ExitLadderRuleRow,
  NodeConfigFormState,
  PtbStopLossBumpLossRuleRow,
} from '@/lib/trade-flow-config-mappers';
import { isEntryTimingProfileRowComplete, isEntryTimingProfileRowEmpty } from '@/lib/trade-flow-config-mappers';

function findIncompleteExitLadderRowError(
  sectionLabel: string,
  rows: ExitLadderRuleRow[]
): string | null {
  for (const [index, row] of rows.entries()) {
    const hasPriceCent = row.priceCent.trim().length > 0;
    const hasSizePct = row.sizePct.trim().length > 0;
    if (hasPriceCent === hasSizePct) continue;

    const missingField = hasPriceCent ? 'Boyut (%)' : 'Fiyat (cent)';
    return `${sectionLabel} - Kademe #${index + 1} icin ${missingField} eksik. Fiyat (cent) ve Boyut (%) birlikte doldurulmali.`;
  }

  return null;
}

function findIncompleteEntryTimingProfileRowError(
  rows: EntryTimingProfileRow[]
): string | null {
  for (const [index, row] of rows.entries()) {
    if (isEntryTimingProfileRowEmpty(row) || isEntryTimingProfileRowComplete(row)) continue;
    return `Entry Timing Profiles - Satir #${index + 1} icin Baslangic ve Bitis saniyeleri birlikte ve gecerli doldurulmali.`;
  }

  return null;
}

function findPtbStopLossBumpLossTableError(
  rows: PtbStopLossBumpLossRuleRow[]
): string | null {
  let previousLossUsd: number | null = null;
  let validRowCount = 0;

  for (const [index, row] of rows.entries()) {
    const lossUsdRaw = row.lossUsd.trim();
    const bumpValueRaw = row.bumpValue.trim();
    const hasLossUsd = lossUsdRaw.length > 0;
    const hasBumpValue = bumpValueRaw.length > 0;

    if (!hasLossUsd && !hasBumpValue) continue;

    if (hasLossUsd !== hasBumpValue) {
      const missingField = hasLossUsd ? 'Bump' : 'Zarar (USD)';
      return `PTB Zarar Bazli Tablo - Kademe #${index + 1} icin ${missingField} eksik. Zarar (USD) ve Bump birlikte doldurulmali.`;
    }

    const lossUsd = Number(lossUsdRaw);
    if (!Number.isFinite(lossUsd) || lossUsd <= 0) {
      return `PTB Zarar Bazli Tablo - Kademe #${index + 1} icin Zarar (USD) 0'dan buyuk sayi olmali.`;
    }

    const bumpValue = Number(bumpValueRaw);
    if (!Number.isFinite(bumpValue) || bumpValue <= 0) {
      return `PTB Zarar Bazli Tablo - Kademe #${index + 1} icin Bump 0'dan buyuk sayi olmali.`;
    }

    if (previousLossUsd != null && lossUsd <= previousLossUsd) {
      return `PTB Zarar Bazli Tablo - Kademe #${index + 1} icin Zarar (USD) onceki satirdan buyuk olmali.`;
    }

    previousLossUsd = lossUsd;
    validRowCount += 1;
  }

  if (validRowCount === 0) {
    return 'PTB Zarar Bazli Tablo - Loss-table modu icin en az bir tam kademe girilmeli.';
  }

  return null;
}

export function validateNodeFormBeforeSave(
  nodeType: string,
  form: NodeConfigFormState
): string | null {
  if (nodeType === 'trigger.market_price') {
    return findIncompleteEntryTimingProfileRowError(form.entryTimingProfileRows || []);
  }
  if (nodeType !== 'action.place_order') return null;

  const pairLockEnabled = (form.fields.mode ?? '').trim().toLowerCase() === 'pair_lock';
  const sections: Array<{ label: string; rows: ExitLadderRuleRow[] }> = [
    {
      label: pairLockEnabled
        ? 'Ilk Bacak Take Profit Kademeleri'
        : 'Take Profit Kademeleri',
      rows: form.tpRuleRows || [],
    },
    {
      label: 'Karsi Bacak Take Profit Kademeleri',
      rows: form.counterLegTpRuleRows || [],
    },
    {
      label: pairLockEnabled
        ? 'Ilk Bacak Stop Loss Kademeleri'
        : 'Stop Loss Kademeleri',
      rows: form.slRuleRows || [],
    },
  ];

  for (const section of sections) {
    const error = findIncompleteExitLadderRowError(section.label, section.rows);
    if (error) return error;
  }

  const ptbStopLossBumpEnabled =
    (form.fields.priceToBeatStopLossBumpEnabled ?? '').trim().toLowerCase() === 'true';
  const ptbStopLossBumpMode =
    (form.fields.priceToBeatStopLossBumpMode ?? '').trim().toLowerCase();
  if (ptbStopLossBumpEnabled && ptbStopLossBumpMode === 'loss_table') {
    return findPtbStopLossBumpLossTableError(form.ptbStopLossBumpLossRuleRows || []);
  }

  return null;
}
