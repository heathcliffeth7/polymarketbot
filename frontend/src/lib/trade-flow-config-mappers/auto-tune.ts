import type { NodeConfigFormState } from './types';
import { isRecord, toStringValue } from './utils';

const AUTO_TUNE_FLAT_KEYS = [
  'autoTuneEnabled',
  'autoTuneMode',
  'autoTuneSampleMarkets',
  'autoTuneMinEligibleMarkets',
  'autoTuneCooldownMarketsAfterAdvice',
  'autoTuneDedupeSameAdviceForMarkets',
] as const;

export function applyAutoTuneFormDefaults(fields: Record<string, string>, cfg: Record<string, unknown>) {
  const nested = isRecord(cfg.autoTune) ? cfg.autoTune : null;
  const source = nested ?? cfg;

  fields.autoTuneEnabled = toStringValue(source.autoTuneEnabled ?? source.enabled);
  fields.autoTuneMode = toStringValue(source.autoTuneMode ?? source.mode);
  fields.autoTuneSampleMarkets = toStringValue(source.autoTuneSampleMarkets ?? source.sampleMarkets);
  fields.autoTuneMinEligibleMarkets = toStringValue(
    source.autoTuneMinEligibleMarkets ?? source.minEligibleMarkets
  );
  fields.autoTuneCooldownMarketsAfterAdvice = toStringValue(
    source.autoTuneCooldownMarketsAfterAdvice ?? source.cooldownMarketsAfterAdvice
  );
  fields.autoTuneDedupeSameAdviceForMarkets = toStringValue(
    source.autoTuneDedupeSameAdviceForMarkets ?? source.dedupeSameAdviceForMarkets
  );

  if (fields.autoTuneEnabled.trim().toLowerCase() === 'true' && !fields.autoTuneMode.trim()) {
    fields.autoTuneMode = 'advice';
  }
}

export function normalizeAutoTuneBuildConfig(
  config: Record<string, unknown>,
  form: NodeConfigFormState
) {
  const enabledRaw = (form.fields.autoTuneEnabled ?? '').trim().toLowerCase();
  const enabled =
    ['true', '1', 'yes', 'y', 'on'].includes(enabledRaw) ? true :
    ['false', '0', 'no', 'n', 'off'].includes(enabledRaw) ? false :
    null;

  for (const key of AUTO_TUNE_FLAT_KEYS) {
    delete config[key];
  }

  if (enabled !== true) {
    delete config.autoTune;
    return;
  }

  const autoTune: Record<string, unknown> = {
    enabled: true,
    mode: 'advice',
  };
  const mode = (form.fields.autoTuneMode ?? '').trim().toLowerCase();
  if (mode) {
    autoTune.mode = mode;
  }
  copyPositiveInteger(form.fields.autoTuneSampleMarkets, autoTune, 'sampleMarkets');
  copyPositiveInteger(form.fields.autoTuneMinEligibleMarkets, autoTune, 'minEligibleMarkets');
  copyPositiveInteger(
    form.fields.autoTuneCooldownMarketsAfterAdvice,
    autoTune,
    'cooldownMarketsAfterAdvice',
    true
  );
  copyPositiveInteger(
    form.fields.autoTuneDedupeSameAdviceForMarkets,
    autoTune,
    'dedupeSameAdviceForMarkets',
    true
  );
  config.autoTune = autoTune;
}

function copyPositiveInteger(
  raw: string | undefined,
  target: Record<string, unknown>,
  key: string,
  allowZero = false
) {
  const trimmed = (raw ?? '').trim();
  if (!trimmed) return;
  const value = Number(trimmed);
  if (!Number.isFinite(value)) return;
  const rounded = Math.floor(value);
  if (allowZero ? rounded >= 0 : rounded > 0) {
    target[key] = rounded;
  }
}
