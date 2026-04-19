import type { PairLockUpstreamTriggerSummary } from '../flow-canvas-utils';

interface TriggerPairLockHintProps {
  visible: boolean;
}

export function TriggerPairLockHint({ visible }: TriggerPairLockHintProps) {
  if (!visible) return null;
  return (
    <div className="rounded-md border border-sky-200 bg-sky-50 px-2 py-2 text-[10px] leading-relaxed text-sky-700">
      Bu trigger tam olarak bir downstream <span className="font-semibold">action.place_order mode=pair_lock</span> node’una baglanmali; ek olarak <span className="font-semibold">action.notify</span> ve <span className="font-semibold">action.telegram_notify</span> node’lari paralel baglanabilir.
    </div>
  );
}

interface PairLockSummarySectionProps {
  visible: boolean;
  primaryOutcomeLabel: string;
  counterOutcomePreview: string;
  upstreamPairLockTrigger: PairLockUpstreamTriggerSummary | null;
}

export function PairLockSummarySection({
  visible,
  primaryOutcomeLabel,
  counterOutcomePreview,
  upstreamPairLockTrigger,
}: PairLockSummarySectionProps) {
  if (!visible) return null;
  return (
    <div className="space-y-1 rounded-md border border-sky-200 bg-sky-50 px-2 py-2 text-[10px] leading-relaxed text-sky-700">
      <p>
        Pair lock aktif. Ana outcome <span className="font-semibold">{primaryOutcomeLabel || 'secilmedi'}</span>
        {counterOutcomePreview ? (
          <> ise karsi bacak otomatik <span className="font-semibold">{counterOutcomePreview}</span> preview ile calisir.</>
        ) : null}
      </p>
      {upstreamPairLockTrigger ? (
        <>
          <p>
            Bagli trigger: <span className="font-semibold">{upstreamPairLockTrigger.nodeKey}</span> | marketMode:{' '}
            <span className="font-semibold">{upstreamPairLockTrigger.marketMode}</span> | cycleWindow:{' '}
            <span className="font-semibold">{upstreamPairLockTrigger.cycleWindowMode}</span>
            {upstreamPairLockTrigger.cycleWindowSecs ? ` (${upstreamPairLockTrigger.cycleWindowSecs}s)` : ''}
            {upstreamPairLockTrigger.cycleWindowStartSec && upstreamPairLockTrigger.cycleWindowEndSec
              ? ` (${upstreamPairLockTrigger.cycleWindowStartSec}-${upstreamPairLockTrigger.cycleWindowEndSec}s)`
              : ''}
          </p>
          <p>
            Pair lock icin ozel aralik varsa <span className="font-semibold">trigger.market_price</span> node&apos;undaki
            <span className="font-semibold"> cycleWindow</span> alanlarindan gelir; pair_lock node&apos;unda ayri zaman araligi yoktur.
          </p>
        </>
      ) : (
        <p className="text-amber-700">
          Pair lock icin dogrudan upstream <span className="font-semibold">trigger.market_price bindingMode=pair_lock_only</span> baglantisi gerekir.
        </p>
      )}
      <p>
        Stop loss yalniz ilk dolan bacak icin calisir. Ikinci bacak dolup pair
        <span className="font-semibold"> locked</span> oldugunda tum SL/PTB-SL yuzeyi otomatik iptal edilir.
      </p>
      <p>
        Retry acik guard&apos;larda ilk bacak secimi hemen fail olmaz; ayni market icinde kosullar
        iyilesince tekrar denenir.
      </p>
      <p>
        Auto-scope acikken primary outcome bos birakilsa bile runtime Up/Down taraflarini mevcut
        buy guard&apos;lariyla yoklar; guard&apos;i gecen ilk bacak primary olarak secilir.
      </p>
    </div>
  );
}
