export type PrimitiveValueType = 'string' | 'number' | 'boolean';
export type ExpressionJoin = 'and' | 'or';
export type ConditionOperator = '>' | '>=' | '<' | '<=' | '==' | '!=';

export interface KeyValueDraft {
  id: string;
  key: string;
  value: string;
  valueType: PrimitiveValueType;
}

export interface ConditionDraft {
  id: string;
  leftVar: string;
  operator: ConditionOperator;
  rightType: PrimitiveValueType;
  rightValue: string;
}

export interface OutcomeConditionRow {
  id: string;
  tokenId: string;
  outcomeLabel: string;
  triggerCondition: string;
  triggerPriceCent: string;
  maxPriceCent: string;
}

export interface DrawdownRuleRow {
  id: string;
  direction: 'down' | 'up';
  lossPct: string;
  durationValue: string;
}

export interface NodeConfigFormState {
  fields: Record<string, string>;
  triggerSizeRows: string[];
  outcomeConditionRows: OutcomeConditionRow[];
  drawdownRuleRows: DrawdownRuleRow[];
  expressionRows: ConditionDraft[];
  expressionJoin: ExpressionJoin;
  expressionSupported: boolean;
  nestedExprMode: boolean;
  nestedExprGroup: import('@/lib/types').ExpressionGroup | null;
  statePatchRows: KeyValueDraft[];
  advancedJson: string;
}

export interface EdgeConditionFormState {
  enabled: boolean;
  conditionRow: ConditionDraft;
  conditionSupported: boolean;
  advancedJson: string;
}

export interface ContextFormState {
  sourceTradeId: string;
  marketSlug: string;
  tokenId: string;
  outcomeLabel: string;
  autoClaimEnabled: boolean;
  extras: KeyValueDraft[];
  advancedJson: string;
}

export interface NodeFieldOption {
  label: string;
  value: string;
}

export interface NodeFieldSchema {
  key: string;
  label: string;
  input: 'text' | 'number' | 'datetime-local' | 'textarea' | 'select';
  help?: string;
  placeholder?: string;
  options?: NodeFieldOption[];
}

const RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME: Record<string, { asset: string; timeframe: string }> = {
  btc_5m_updown: { asset: 'btc', timeframe: '5m' },
  btc_15m_updown: { asset: 'btc', timeframe: '15m' },
  eth_5m_updown: { asset: 'eth', timeframe: '5m' },
  eth_15m_updown: { asset: 'eth', timeframe: '15m' },
  sol_5m_updown: { asset: 'sol', timeframe: '5m' },
  sol_15m_updown: { asset: 'sol', timeframe: '15m' },
  xrp_5m_updown: { asset: 'xrp', timeframe: '5m' },
  xrp_15m_updown: { asset: 'xrp', timeframe: '15m' },
};
const QUICK_PRESET_BUY_SELL_REF_KEYS = new Set([
  'preset_sell_current_position',
  'preset_buy_current_position',
]);
const QUICK_PRESET_BUY_SELL_KINDS = new Set([
  'sell_current_position',
  'buy_current_position',
]);
const PRESET_PLACE_ORDER_KINDS = new Set([
  'place_order',
  ...QUICK_PRESET_BUY_SELL_KINDS,
]);
const TRIGGER_MARKET_ONCE_SCOPE_VERSION = 2;

export function isPresetBuySellPlaceOrderMarker(presetKind: unknown, refKey: unknown): boolean {
  const kind = toStringValue(presetKind).trim().toLowerCase();
  if (QUICK_PRESET_BUY_SELL_KINDS.has(kind)) return true;
  const ref = toStringValue(refKey).trim().toLowerCase();
  return QUICK_PRESET_BUY_SELL_REF_KEYS.has(ref);
}

export function isPresetPlaceOrderMarker(presetKind: unknown, refKey: unknown): boolean {
  const kind = toStringValue(presetKind).trim().toLowerCase();
  if (PRESET_PLACE_ORDER_KINDS.has(kind)) return true;
  const ref = toStringValue(refKey).trim().toLowerCase();
  return ref.startsWith('preset_');
}

function normalizeResolveMarketScope(scope: unknown): { asset: string; timeframe: string } | null {
  const key = toStringValue(scope).trim().toLowerCase();
  if (!key) return null;
  return RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[key] || null;
}

function toResolveMarketScope(assetRaw: unknown, timeframeRaw: unknown): string | null {
  const asset = toStringValue(assetRaw).trim().toLowerCase();
  const timeframe = toStringValue(timeframeRaw).trim().toLowerCase();
  if (!asset || !timeframe) return null;
  const scope = `${asset}_${timeframe}_updown`;
  return RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[scope] ? scope : null;
}

const CONDITION_OPERATORS: ConditionOperator[] = ['>', '>=', '<', '<=', '==', '!='];

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

export function safeJsonStringify(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function createId(prefix: string): string {
  return `${prefix}_${Math.random().toString(36).slice(2, 10)}`;
}

function toStringValue(value: unknown): string {
  if (value == null) return '';
  if (typeof value === 'string') return value;
  if (typeof value === 'number' || typeof value === 'boolean') return String(value);
  return '';
}

function toCentStringValue(centValue: unknown, legacyDecimalValue?: unknown): string {
  const cent = toStringValue(centValue).trim();
  if (cent) return cent;

  const legacyDecimal = Number(toStringValue(legacyDecimalValue).trim());
  if (Number.isFinite(legacyDecimal) && legacyDecimal > 0 && legacyDecimal <= 1) {
    return String(Math.round(legacyDecimal * 100));
  }

  return '';
}

function toTriggerMarketOnceScopeVersion(value: unknown): number {
  const parsed = Number(toStringValue(value).trim());
  if (!Number.isFinite(parsed)) return 0;
  return Math.trunc(parsed);
}

function resolveTriggerMarketOnceScope(
  cfg: Record<string, unknown>,
  marketMode: 'auto_scope' | 'fixed',
  repeatMode: 'once' | 'loop'
): 'run' | 'market' {
  const onceScopeRaw = toStringValue(cfg.onceScope).trim().toLowerCase();
  const onceScopeVersion = toTriggerMarketOnceScopeVersion(cfg.onceScopeVersion);

  if (
    marketMode === 'auto_scope' &&
    repeatMode === 'once' &&
    onceScopeVersion < TRIGGER_MARKET_ONCE_SCOPE_VERSION
  ) {
    return 'market';
  }

  return onceScopeRaw === 'market' ? 'market' : 'run';
}

function valueTypeOf(value: unknown): PrimitiveValueType {
  if (typeof value === 'number') return 'number';
  if (typeof value === 'boolean') return 'boolean';
  return 'string';
}

function parsePrimitive(value: string, valueType: PrimitiveValueType): unknown {
  if (valueType === 'number') {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  if (valueType === 'boolean') {
    if (value.trim().toLowerCase() === 'true') return true;
    if (value.trim().toLowerCase() === 'false') return false;
    return null;
  }
  return value;
}

function toBooleanValue(value: unknown): boolean {
  if (typeof value === 'boolean') return value;
  if (typeof value === 'number') return Number.isFinite(value) && value !== 0;
  if (typeof value !== 'string') return false;

  const normalized = value.trim().toLowerCase();
  return ['true', '1', 'yes', 'y', 'on'].includes(normalized);
}

export function createEmptyConditionDraft(): ConditionDraft {
  return {
    id: createId('cond'),
    leftVar: 'market_price',
    operator: '<=',
    rightType: 'number',
    rightValue: '50',
  };
}

export function createEmptyKeyValueDraft(): KeyValueDraft {
  return {
    id: createId('kv'),
    key: '',
    value: '',
    valueType: 'string',
  };
}

export function createEmptyOutcomeConditionRow(): OutcomeConditionRow {
  return {
    id: createId('oc'),
    tokenId: '',
    outcomeLabel: '',
    triggerCondition: '',
    triggerPriceCent: '',
    maxPriceCent: '',
  };
}

export function createEmptyDrawdownRuleRow(): DrawdownRuleRow {
  return { id: createId('dr'), direction: 'down', lossPct: '', durationValue: '' };
}

export const NODE_FIELD_SCHEMAS: Record<string, NodeFieldSchema[]> = {
  'trigger.market_price': [
    {
      key: 'marketMode',
      label: 'Market Modu',
      input: 'select',
      options: [
        { label: 'Sabit (fixed)', value: 'fixed' },
        { label: 'Otomatik Scope (auto_scope)', value: 'auto_scope' },
      ],
    },
    { key: 'marketSlug', label: 'Market Slug', input: 'text' },
    {
      key: 'marketScope',
      label: 'Market Scope',
      input: 'select',
      options: [
        { label: 'BTC 5m', value: 'btc_5m_updown' },
        { label: 'BTC 15m', value: 'btc_15m_updown' },
        { label: 'ETH 5m', value: 'eth_5m_updown' },
        { label: 'ETH 15m', value: 'eth_15m_updown' },
        { label: 'SOL 5m', value: 'sol_5m_updown' },
        { label: 'SOL 15m', value: 'sol_15m_updown' },
        { label: 'XRP 5m', value: 'xrp_5m_updown' },
        { label: 'XRP 15m', value: 'xrp_15m_updown' },
      ],
    },
    {
      key: 'marketSelection',
      label: 'Secim Stratejisi',
      input: 'select',
      options: [{ label: 'latest_by_slug', value: 'latest_by_slug' }],
    },
    {
      key: 'protectionMode',
      label: 'Underlying Koruma',
      input: 'select',
      options: [
        { label: 'Kapali (off)', value: 'off' },
        { label: 'Asset teyidi', value: 'underlying_confirm' },
      ],
      help: 'Koruma secilen auto_scope assetine otomatik baglanir. BTC scope BTC ile, XRP scope XRP ile dogrulanir.',
    },
    {
      key: 'protectionPreset',
      label: 'Koruma Preseti',
      input: 'select',
      options: [
        { label: 'Loose', value: 'loose' },
        { label: 'Balanced', value: 'balanced' },
        { label: 'Strict', value: 'strict' },
      ],
      help: 'Balanced varsayilandir. Ayrica manuel asset secimi yoktur; secilen scope kullanilir.',
    },
    {
      key: 'cycleWindowMode',
      label: 'Cycle Pencere Modu',
      input: 'select',
      options: [
        { label: 'Tamamı (kapalı)', value: 'off' },
        { label: 'İlk N saniye (first)', value: 'first' },
        { label: 'Son N saniye (last)', value: 'last' },
      ],
      help: 'Cycle icinde sadece belirli bir zaman penceresinde tetik degerlendirilir. `last` modunda pencereye zaten esik ustunde girerse tetiklemez; bu pencere icinde gercek cross gerekir.',
    },
    {
      key: 'cycleWindowSecs',
      label: 'Pencere Süresi (saniye)',
      input: 'number',
      help: 'Kac saniye boyunca tetik degerlendirilecek. Or: 5m cycle, last 60 -> son 60sn. `last` modunda bu pencere icinde esigin altindan ustune gercek gecis aranir.',
    },
    {
      key: 'repeatMode',
      label: 'Tetik Modu',
      input: 'select',
      options: [
        { label: '1 Kere (once)', value: 'once' },
        { label: 'Döngü (loop)', value: 'loop' },
      ],
    },
    {
      key: 'onceScope',
      label: 'Once Scope',
      input: 'select',
      options: [
        { label: 'Run', value: 'run' },
        { label: 'Market', value: 'market' },
      ],
      help: '`run` ilk basarili tetikten sonra workflow run boyunca tekrar almaz. `market` her yeni auto_scope markette bir kez daha tetikleyebilir. Auto-scope + once varsayilani markettir.',
    },
    { key: 'minIntervalMs', label: 'Kontrol Aralığı (ms)', input: 'number', help: 'Varsayılan: 10000 (10sn). Minimum: 250ms.' },
    { key: 'confirmationMs', label: 'Onay Süresi (ms)', input: 'number', help: 'Cross sonrası fiyatın eşikte kalması gereken süre. Boş = onay kapalı, 0 = anında tetik.' },
    {
      key: 'priceMode',
      label: 'Fiyat Kaynağı',
      input: 'select',
      options: [
        { label: 'midpoint (önerilen)', value: 'midpoint' },
        { label: 'raw trade', value: 'raw' },
        { label: 'best bid (satış için)', value: 'best_bid' },
        { label: 'best ask (alış için)', value: 'best_ask' },
      ],
      help: 'midpoint: best bid/ask ortalaması (daha stabil). raw: WS trade/price_changes (daha oynak). best_bid: en iyi alım fiyatı (satış tetikleyici). best_ask: en iyi satım fiyatı (alış tetikleyici).',
    },
  ],
  'trigger.sell_progress': [
    { key: 'sourceTradeId', label: 'Source Trade ID', input: 'number' },
    { key: 'minProgressPct', label: 'Minimum İlerleme (%)', input: 'number' },
    { key: 'varKey', label: 'Değişken Anahtarı', input: 'text', placeholder: 'sell_progress_pct' },
    { key: 'minIntervalMs', label: 'Minimum Interval (ms)', input: 'number' },
  ],
  'trigger.open_positions': [
    { key: 'sourceTradeId', label: 'Source Trade ID', input: 'number' },
    { key: 'marketSlug', label: 'Market Slug', input: 'text' },
    { key: 'tokenId', label: 'Token ID', input: 'text' },
    { key: 'outcomeLabel', label: 'Outcome Label', input: 'text' },
    {
      key: 'triggerCondition',
      label: 'Fiyat Tetik Koşulu',
      input: 'select',
      options: [
        { label: 'Yok', value: '' },
        { label: 'cross_above', value: 'cross_above' },
        { label: 'cross_below', value: 'cross_below' },
      ],
    },
    {
      key: 'triggerPriceCent',
      label: 'Fiyat Eşiği (cent)',
      input: 'number',
      help: 'Örn: 70 girersen 70c.',
    },
    {
      key: 'maxPriceCent',
      label: 'Tavan Fiyati (cent)',
      input: 'number',
      help: 'Opsiyonel. Bossa ust limit uygulanmaz.',
    },
    {
      key: 'minIntervalMs',
      label: 'Minimum Interval (ms)',
      input: 'number',
      help: 'Fiyat tetik koşulu aktifse kullanılmaz; websocket event tetikler.',
    },
  ],
  'trigger.position_drawdown': [
    { key: 'marketSlug', label: 'Market Slug', input: 'text' },
    { key: 'outcomeLabel', label: 'Outcome Label', input: 'text' },
    {
      key: 'entryPriceCent',
      label: 'Entry Fiyati (cent)',
      input: 'number',
      help: 'Pozisyona bakmadan drawdown hesabi bu entry fiyatina gore yapilir. Ornek: 80 => $0.80',
    },
    {
      key: 'combineMode',
      label: 'Kural Birlesimi',
      input: 'select',
      options: [
        { label: 'Otomatik', value: '' },
        { label: 'AND (hepsi)', value: 'and' },
        { label: 'OR (herhangi biri)', value: 'or' },
      ],
      help: 'Bos birakirsan: tek kural varsa tek kural, birden fazla kural varsa OR uygulanir.',
    },
    {
      key: 'minIntervalMs',
      label: 'Minimum Interval (ms)',
      input: 'number',
      help: 'Varsayilan 250ms. Minimum 250ms.',
    },
    { key: 'varPrefix', label: 'Degisken Prefix', input: 'text', placeholder: 'drawdown' },
  ],
  'trigger.time_window': [
    { key: 'startAt', label: 'Başlangıç Zamanı', input: 'datetime-local' },
    { key: 'endAt', label: 'Bitiş Zamanı', input: 'datetime-local' },
    { key: 'varKey', label: 'Değişken Anahtarı', input: 'text', placeholder: 'time_window_open' },
    { key: 'minIntervalMs', label: 'Minimum Interval (ms)', input: 'number' },
  ],
  'logic.if': [
    { key: 'comment', label: 'Açıklama', input: 'textarea' },
  ],
  'logic.switch': [
    { key: 'comment', label: 'Açıklama', input: 'textarea' },
  ],
  'logic.delay': [
    { key: 'delayMs', label: 'Gecikme (ms)', input: 'number' },
  ],
  'logic.retry': [
    { key: 'maxAttempts', label: 'Maksimum Deneme', input: 'number' },
  ],
  'action.resolve_market': [
    {
      key: 'asset',
      label: 'Asset',
      input: 'select',
      options: [
        { label: 'BTC', value: 'btc' },
        { label: 'ETH', value: 'eth' },
        { label: 'SOL', value: 'sol' },
        { label: 'XRP', value: 'xrp' },
      ],
    },
    {
      key: 'timeframe',
      label: 'Timeframe',
      input: 'select',
      options: [
        { label: '5m', value: '5m' },
        { label: '15m', value: '15m' },
      ],
      help: 'Market slug bu secime gore otomatik hesaplanir.',
    },
    {
      key: 'selection',
      label: 'Secim Stratejisi',
      input: 'select',
      options: [{ label: 'latest_by_slug', value: 'latest_by_slug' }],
    },
    {
      key: 'outcomeLabel',
      label: 'Outcome',
      input: 'select',
      options: [
        { label: 'yes', value: 'yes' },
        { label: 'no', value: 'no' },
      ],
      help: 'tokenId secimi bu outcome alanina gore yapilir.',
    },
    {
      key: 'failOnMissingMarket',
      label: 'Market Yoksa Hata',
      input: 'select',
      options: [
        { label: 'true', value: 'true' },
        { label: 'false', value: 'false' },
      ],
    },
    {
      key: 'requireYesNoTokens',
      label: 'YES/NO Token Zorunlu',
      input: 'select',
      options: [
        { label: 'true', value: 'true' },
        { label: 'false', value: 'false' },
      ],
    },
    {
      key: 'requireTokenId',
      label: 'Secilen Token Zorunlu',
      input: 'select',
      options: [
        { label: 'true', value: 'true' },
        { label: 'false', value: 'false' },
      ],
    },
    { key: 'varPrefix', label: 'Degisken Prefix', input: 'text', placeholder: 'resolved_market' },
  ],
  'action.dual_dca': [
    { key: 'sourceTradeId', label: 'Source Trade ID', input: 'number' },
    {
      key: 'asset',
      label: 'Coin',
      input: 'select',
      options: [
        { label: 'BTC', value: 'btc' },
        { label: 'ETH', value: 'eth' },
        { label: 'SOL', value: 'sol' },
        { label: 'XRP', value: 'xrp' },
      ],
    },
    {
      key: 'timeframe',
      label: 'Market Period',
      input: 'select',
      options: [
        { label: '5m', value: '5m' },
        { label: '15m', value: '15m' },
      ],
    },
    {
      key: 'sideMode',
      label: 'Side',
      input: 'select',
      options: [
        { label: 'all', value: 'all' },
        { label: 'up', value: 'up' },
        { label: 'down', value: 'down' },
      ],
    },
    {
      key: 'baseSizing',
      label: 'Base Sizing',
      input: 'select',
      options: [
        { label: 'shares', value: 'shares' },
        { label: 'usdc', value: 'usdc' },
      ],
    },
    { key: 'baseShares', label: 'Base Shares', input: 'number' },
    { key: 'baseUsdc', label: 'Base USDC', input: 'number' },
    {
      key: 'basePriceUsdc',
      label: 'Base Price (USDC)',
      input: 'number',
      help: '0.55 gibi bir deger girersen ilk alim da bu fiyata gelince tetiklenir. Bos birakirsan ilk alim aninda calisir.',
    },
    { key: 'dcaLevels', label: 'DCA Levels (Base Haric)', input: 'number' },
    { key: 'nearStep', label: 'Near Step', input: 'number' },
    { key: 'stepMult', label: 'Step Mult.', input: 'number' },
    { key: 'sizeMult', label: 'Size Mult.', input: 'number' },
    { key: 'minPriceDistanceCent', label: 'Min Price Distance (cent)', input: 'number' },
    { key: 'cutoffMin', label: 'Cutoff Min', input: 'number' },
    { key: 'tpProfitPct', label: 'TP Profit (USDC)', input: 'number' },
    { key: 'slLossPct', label: 'SL Loss (USDC)', input: 'number' },
    { key: 'slSpreadPct', label: 'SL Spread (USDC)', input: 'number' },
    { key: 'refKey', label: 'Reference Key', input: 'text' },
  ],
  'action.place_order': [
    { key: 'sourceTradeId', label: 'Source Trade ID', input: 'number' },
    {
      key: 'side',
      label: 'Yön',
      input: 'select',
      options: [
        { label: 'buy', value: 'buy' },
        { label: 'sell', value: 'sell' },
      ],
    },
    {
      key: 'executionMode',
      label: 'Islem Modu',
      input: 'select',
      options: [
        { label: 'market (IOC)', value: 'market' },
        { label: 'limit (GTC)', value: 'limit' },
      ],
      help: 'market secimi piyasa benzeri davranis icin IOC + agresif fiyat kullanir.',
    },
    { key: 'marketSlug', label: 'Market Slug', input: 'text' },
    { key: 'tokenId', label: 'Token ID', input: 'text' },
    { key: 'outcomeLabel', label: 'Outcome Label', input: 'text' },
    {
      key: 'sizeMode',
      label: 'Tutar Modu',
      input: 'select',
      options: [
        { label: 'USDC', value: 'usdc' },
        { label: '% (Pozisyon Yüzdesi)', value: 'pct' },
      ],
      help: "pct seçersen tutar Source Trade notional'ının yüzdesi olarak hesaplanır.",
    },
    { key: 'sizeUsdc', label: 'Tutar (USDC)', input: 'number' },
    { key: 'sizePct', label: 'Tutar (%)', input: 'number', help: 'Geçerli aralık: 0 < % <= 100.' },
    { key: 'minPriceDistanceCent', label: 'Minimum Fiyat Mesafesi (cent)', input: 'number' },
    { key: 'maxTriggers', label: 'Maksimum Tetikleme', input: 'number' },
    {
      key: 'kind',
      label: 'Emir Tipi',
      input: 'select',
      options: [
        { label: 'immediate', value: 'immediate' },
        { label: 'conditional', value: 'conditional' },
      ],
    },
    {
      key: 'triggerCondition',
      label: 'Tetik Koşulu',
      input: 'select',
      options: [
        { label: 'Yok', value: '' },
        { label: 'cross_above', value: 'cross_above' },
        { label: 'cross_below', value: 'cross_below' },
      ],
    },
    { key: 'triggerPrice', label: 'Tetik Fiyatı', input: 'number' },
    { key: 'expiresAt', label: 'Bitiş Zamanı', input: 'datetime-local' },
    {
      key: 'tpEnabled',
      label: 'Take Profit',
      input: 'select',
      options: [
        { label: 'Kapali', value: 'false' },
        { label: 'Aktif', value: 'true' },
      ],
      help: 'Buy emri doldurulunca belirtilen fiyatta otomatik IOC sell emri olusturur.',
    },
    { key: 'tpPriceCent', label: 'TP Fiyat (cent)', input: 'number', help: 'Hedef satis fiyati (1-99 cent). Fiyat bu seviyeyi gecince marketten IOC ile satilir.' },
    {
      key: 'slEnabled',
      label: 'Stop Loss',
      input: 'select',
      options: [
        { label: 'Kapali', value: 'false' },
        { label: 'Aktif', value: 'true' },
      ],
      help: 'Buy emri doldurulunca belirtilen stop seviyesinde otomatik IOC sell emri olusturur.',
    },
    { key: 'slPriceCent', label: 'SL Fiyat (cent)', input: 'number', help: 'Stop satis fiyati (1-99 cent). Fiyat bu seviyenin altina inince marketten IOC ile satilir.' },
    { key: 'refKey', label: 'Reference Key', input: 'text' },
  ],
  'action.cancel_order': [
    { key: 'builderOrderId', label: 'Builder Order ID', input: 'number' },
    { key: 'targetRef', label: 'Target Ref', input: 'text' },
  ],
  'action.update_order': [
    { key: 'builderOrderId', label: 'Builder Order ID', input: 'number' },
    { key: 'targetRef', label: 'Target Ref', input: 'text' },
    { key: 'minPriceDistanceCent', label: 'Minimum Fiyat Mesafesi (cent)', input: 'number' },
    { key: 'maxTriggers', label: 'Maksimum Tetikleme', input: 'number' },
  ],
  'action.set_state': [],
  'action.notify': [
    { key: 'channel', label: 'Kanal', input: 'text' },
    { key: 'message', label: 'Mesaj', input: 'textarea' },
  ],
  'action.telegram_notify': [
    {
      key: 'chatId',
      label: 'Chat ID (Opsiyonel Override)',
      input: 'text',
      placeholder: 'Bos birakirsan Settings -> Telegram chat_id kullanilir',
    },
    { key: 'message', label: 'Mesaj', input: 'textarea', placeholder: 'Tetik: {{vars.trigger_1_price}}' },
  ],
};

const NUMERIC_KEYS = new Set([
  'pollIntervalMs',
  'minIntervalMs',
  'triggerPrice',
  'triggerPriceCent',
  'maxPriceCent',
  'sourceTradeId',
  'minProgressPct',
  'minPositionQty',
  'delayMs',
  'maxAttempts',
  'sizeUsdc',
  'sizePct',
  'targetNotionalUsdc',
  'minPriceDistanceCent',
  'maxTriggers',
  'builderOrderId',
  'baseShares',
  'baseUsdc',
  'basePriceUsdc',
  'dcaLevels',
  'nearStep',
  'stepMult',
  'sizeMult',
  'cutoffMin',
  'tpProfitPct',
  'slLossPct',
  'slSpreadPct',
  'confirmationMs',
  'entryPriceCent',
  'tpPriceCent',
  'slPriceCent',
]);

const BOOLEAN_KEYS = new Set([
  'failOnMissingMarket',
  'requireYesNoTokens',
  'requireTokenId',
  'tpEnabled',
  'slEnabled',
]);

function toDateTimeLocalString(value: unknown): string {
  const raw = toStringValue(value).trim();
  if (!raw) return '';
  if (/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(raw)) return raw;

  const parsed = new Date(raw);
  if (Number.isNaN(parsed.getTime())) return '';
  const year = parsed.getFullYear();
  const month = `${parsed.getMonth() + 1}`.padStart(2, '0');
  const day = `${parsed.getDate()}`.padStart(2, '0');
  const hour = `${parsed.getHours()}`.padStart(2, '0');
  const minute = `${parsed.getMinutes()}`.padStart(2, '0');
  return `${year}-${month}-${day}T${hour}:${minute}`;
}

function parseSimpleCondition(input: unknown): Omit<ConditionDraft, 'id'> | null {
  if (!isRecord(input)) return null;

  const operators = CONDITION_OPERATORS.filter((operator) => Object.prototype.hasOwnProperty.call(input, operator));
  if (operators.length !== 1) return null;

  const operator = operators[0];
  const rawOperands = input[operator];
  if (!Array.isArray(rawOperands) || rawOperands.length !== 2) return null;

  const left = rawOperands[0];
  const right = rawOperands[1];
  if (!isRecord(left) || typeof left.var !== 'string') return null;

  const rightType = valueTypeOf(right);
  return {
    leftVar: left.var,
    operator,
    rightType,
    rightValue: toStringValue(right),
  };
}

function buildSimpleCondition(draft: ConditionDraft): Record<string, unknown> {
  const primitive = parsePrimitive(draft.rightValue, draft.rightType);
  const normalizedRight =
    primitive == null
      ? draft.rightType === 'number'
        ? 0
        : draft.rightType === 'boolean'
          ? false
          : ''
      : primitive;

  return {
    [draft.operator]: [{ var: draft.leftVar || 'market_price' }, normalizedRight],
  };
}

function parseExpressionDraft(
  expression: unknown
): { rows: ConditionDraft[]; join: ExpressionJoin; supported: boolean } {
  const parsedSingle = parseSimpleCondition(expression);
  if (parsedSingle) {
    return {
      rows: [{ id: createId('expr'), ...parsedSingle }],
      join: 'and',
      supported: true,
    };
  }

  if (isRecord(expression)) {
    const join = Array.isArray(expression.and) ? 'and' : Array.isArray(expression.or) ? 'or' : null;
    if (join) {
      const expressions = (expression[join] as unknown[]) || [];
      const rows: ConditionDraft[] = [];
      for (const item of expressions) {
        const parsed = parseSimpleCondition(item);
        if (!parsed) {
          return {
            rows: [createEmptyConditionDraft()],
            join: 'and',
            supported: false,
          };
        }
        rows.push({ id: createId('expr'), ...parsed });
      }
      if (rows.length > 0) {
        return {
          rows,
          join,
          supported: true,
        };
      }
    }
  }

  return {
    rows: [createEmptyConditionDraft()],
    join: 'and',
    supported: false,
  };
}

function buildExpression(rows: ConditionDraft[], join: ExpressionJoin): Record<string, unknown> {
  const validRows = rows.filter((row) => row.leftVar.trim());
  if (validRows.length === 0) {
    return { '==': [1, 1] };
  }
  if (validRows.length === 1) {
    return buildSimpleCondition(validRows[0]);
  }
  return {
    [join]: validRows.map((row) => buildSimpleCondition(row)),
  };
}

function objectToRows(value: unknown): KeyValueDraft[] {
  if (!isRecord(value)) return [];
  return Object.entries(value).map(([key, rawValue]) => ({
    id: createId('kv'),
    key,
    value: toStringValue(rawValue),
    valueType: valueTypeOf(rawValue),
  }));
}

function parseNumberArrayToStringRows(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.map((item) => toStringValue(item).trim());
}

export function buildObjectFromKeyValueDrafts(rows: KeyValueDraft[]): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const row of rows) {
    const key = row.key.trim();
    if (!key) continue;
    const parsed = parsePrimitive(row.value, row.valueType);
    if (parsed == null) continue;
    result[key] = parsed;
  }
  return result;
}

export function parseNodeConfigToForm(nodeType: string, config: unknown): NodeConfigFormState {
  const cfg = isRecord(config) ? config : {};
  const fields: Record<string, string> = {};
  let triggerSizeRows: string[] = [];
  for (const field of NODE_FIELD_SCHEMAS[nodeType] || []) {
    fields[field.key] =
      field.input === 'datetime-local'
      ? toDateTimeLocalString(cfg[field.key])
      : toStringValue(cfg[field.key]);
  }
  if (nodeType === 'action.telegram_notify') {
    fields.botToken = toStringValue(cfg.botToken);
  }
  if (nodeType === 'action.place_order') {
    if (!fields.tpPriceCent.trim()) {
      const legacyTpPrice = Number(cfg.tpPrice);
      if (Number.isFinite(legacyTpPrice) && legacyTpPrice > 0 && legacyTpPrice <= 1) {
        fields.tpPriceCent = String(Math.round(legacyTpPrice * 100));
      }
    }
    if (!fields.slPriceCent.trim()) {
      const legacySlPrice = Number(cfg.slPrice);
      if (Number.isFinite(legacySlPrice) && legacySlPrice > 0 && legacySlPrice <= 1) {
        fields.slPriceCent = String(Math.round(legacySlPrice * 100));
      }
    }
    if (!fields.sizePct.trim()) {
      fields.sizePct = toStringValue(cfg.sizePercent);
    }
    fields.presetKind = toStringValue(fields.presetKind || cfg.presetKind);
    const existingMode = String(fields.sizeMode ?? '').trim().toLowerCase();
    if (existingMode !== 'usdc' && existingMode !== 'pct') {
      const hasPct =
        typeof cfg.sizePct === 'number' ||
        (typeof cfg.sizePct === 'string' && cfg.sizePct.trim().length > 0) ||
        typeof cfg.sizePercent === 'number' ||
        (typeof cfg.sizePercent === 'string' && cfg.sizePercent.trim().length > 0);
      fields.sizeMode = hasPct ? 'pct' : 'usdc';
    }
    const parsedRows = parseNumberArrayToStringRows(cfg.triggerSizes).slice(0, 20);
    const parsedMaxTriggers = Number(fields.maxTriggers ?? '');
    const rowTarget =
      Number.isFinite(parsedMaxTriggers) && parsedMaxTriggers > 1
        ? Math.min(20, Math.floor(parsedMaxTriggers))
        : 0;
    triggerSizeRows =
      rowTarget > 0
        ? Array.from({ length: rowTarget }, (_, index) => parsedRows[index] ?? '')
        : parsedRows;

    const isPresetPlaceOrder = isPresetPlaceOrderMarker(
      fields.presetKind,
      fields.refKey || cfg.refKey
    );
    const isPresetBuySell = isPresetBuySellPlaceOrderMarker(
      fields.presetKind,
      fields.refKey || cfg.refKey
    );
    if (isPresetPlaceOrder) {
      if (!(fields.presetKind ?? '').trim()) {
        const ref = toStringValue(fields.refKey || cfg.refKey).trim().toLowerCase();
        if (ref === 'preset_sell_current_position') {
          fields.presetKind = 'sell_current_position';
        } else if (ref === 'preset_buy_current_position') {
          fields.presetKind = 'buy_current_position';
        } else if (ref === 'preset_place_order') {
          fields.presetKind = 'place_order';
        }
      }
      fields.kind = 'immediate';
      fields.triggerCondition = '';
      fields.triggerPrice = '';
      fields.triggerPriceCent = '';
      if (isPresetBuySell) {
        fields.executionMode = 'market';
      }
    }
  }
  if (nodeType === 'action.resolve_market') {
    const legacy = normalizeResolveMarketScope(cfg.marketScope);
    if (!fields.asset.trim()) {
      fields.asset = toStringValue(cfg.asset).trim().toLowerCase() || legacy?.asset || 'btc';
    }
    if (!fields.timeframe.trim()) {
      fields.timeframe =
        toStringValue(cfg.timeframe).trim().toLowerCase() || legacy?.timeframe || '5m';
    }
    if (!fields.selection.trim()) fields.selection = 'latest_by_slug';
    if (!fields.outcomeLabel.trim()) {
      fields.outcomeLabel = toStringValue(cfg.outcomeLabel).trim().toLowerCase() || 'yes';
    }
    if (!fields.failOnMissingMarket.trim()) fields.failOnMissingMarket = 'true';
    if (!fields.requireYesNoTokens.trim()) fields.requireYesNoTokens = 'true';
    if (!fields.requireTokenId.trim()) fields.requireTokenId = 'true';
    if (!fields.varPrefix.trim()) fields.varPrefix = 'resolved_market';
  }
  if (nodeType === 'action.dual_dca') {
    const asset =
      toStringValue(cfg.asset).trim().toLowerCase() ||
      toStringValue(cfg.coin).trim().toLowerCase();
    const timeframeRaw =
      toStringValue(cfg.timeframe).trim().toLowerCase() ||
      toStringValue(cfg.marketPeriod).trim().toLowerCase();
    const timeframe =
      timeframeRaw === '5' || timeframeRaw === '5min' || timeframeRaw === '5 min'
        ? '5m'
        : timeframeRaw === '15' || timeframeRaw === '15min' || timeframeRaw === '15 min'
          ? '15m'
          : timeframeRaw;
    const sideModeRaw =
      toStringValue(cfg.sideMode).trim().toLowerCase() ||
      toStringValue(cfg.side).trim().toLowerCase();
    const sideMode =
      sideModeRaw === 'up' || sideModeRaw === 'down' || sideModeRaw === 'all'
        ? sideModeRaw
        : '';
    const baseSizingRaw =
      toStringValue(cfg.baseSizing).trim().toLowerCase() ||
      toStringValue(cfg.baseSizeMode).trim().toLowerCase();

    if (!fields.asset.trim() && asset) fields.asset = asset;
    if (!fields.timeframe.trim() && timeframe) fields.timeframe = timeframe;
    if (!fields.sideMode.trim() && sideMode) fields.sideMode = sideMode;
    if (
      !fields.baseSizing.trim() &&
      (baseSizingRaw === 'usdc' || baseSizingRaw === 'shares')
    ) {
      fields.baseSizing = baseSizingRaw;
    }
    if (!fields.tpProfitPct.trim()) {
      fields.tpProfitPct = toStringValue(cfg.tpProfitPct ?? cfg.tpProfit);
    }
    if (!fields.slLossPct.trim()) {
      fields.slLossPct = toStringValue(cfg.slLossPct ?? cfg.slLoss);
    }
    if (!fields.slSpreadPct.trim()) {
      fields.slSpreadPct = toStringValue(cfg.slSpreadPct ?? cfg.slSpread);
    }
  }

  if (nodeType === 'trigger.market_price') {
    const marketModeRaw = toStringValue(cfg.marketMode).trim().toLowerCase();
    const marketMode = marketModeRaw === 'auto_scope' ? 'auto_scope' : 'fixed';
    fields.marketMode = marketMode;
    const priceModeRaw = toStringValue(cfg.priceMode).trim().toLowerCase();
    const validPriceModes = ['midpoint', 'raw', 'best_bid', 'best_ask'];
    fields.priceMode = validPriceModes.includes(priceModeRaw) ? priceModeRaw : 'midpoint';

    const scopeRaw = toStringValue(cfg.marketScope).trim().toLowerCase();
    if (scopeRaw && RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[scopeRaw]) {
      fields.marketScope = scopeRaw;
    }

    const selectionRaw = toStringValue(cfg.marketSelection).trim().toLowerCase();
    fields.marketSelection = selectionRaw || 'latest_by_slug';
    const protectionModeRaw = toStringValue(cfg.protectionMode).trim().toLowerCase();
    fields.protectionMode =
      protectionModeRaw === 'underlying_confirm' ? 'underlying_confirm' : 'off';
    const protectionPresetRaw = toStringValue(cfg.protectionPreset).trim().toLowerCase();
    fields.protectionPreset =
      protectionPresetRaw === 'loose' ||
      protectionPresetRaw === 'balanced' ||
      protectionPresetRaw === 'strict'
        ? protectionPresetRaw
        : 'balanced';

    const repeatModeRaw = toStringValue(fields.repeatMode || cfg.repeatMode).trim().toLowerCase();
    fields.repeatMode = repeatModeRaw === 'once' ? 'once' : 'loop';
    fields.onceScope = resolveTriggerMarketOnceScope(cfg, marketMode, fields.repeatMode as 'once' | 'loop');

    const cycleWindowModeRaw = toStringValue(cfg.cycleWindowMode).trim().toLowerCase();
    if (cycleWindowModeRaw === 'first' || cycleWindowModeRaw === 'last') {
      fields.cycleWindowMode = cycleWindowModeRaw;
    } else {
      fields.cycleWindowMode = 'off';
    }
    fields.cycleWindowSecs = toStringValue(cfg.cycleWindowSecs);

  }

  if (nodeType === 'trigger.open_positions') {
    fields.maxPriceCent = toCentStringValue(fields.maxPriceCent || cfg.maxPriceCent, cfg.maxPrice);
  }

  const outcomeConditionRows: OutcomeConditionRow[] = [];
  let drawdownRuleRows: DrawdownRuleRow[] = [];
  if (nodeType === 'trigger.open_positions' || nodeType === 'trigger.market_price') {
    if (Array.isArray(cfg.outcomeConditions)) {
      for (const item of cfg.outcomeConditions as Record<string, unknown>[]) {
        if (!isRecord(item)) continue;
        outcomeConditionRows.push({
          id: createId('oc'),
          tokenId: toStringValue(item.tokenId),
          outcomeLabel: toStringValue(item.outcomeLabel),
          triggerCondition: toStringValue(item.triggerCondition),
          triggerPriceCent: toStringValue(item.triggerPriceCent),
          maxPriceCent: toCentStringValue(item.maxPriceCent, item.maxPrice),
        });
      }
    } else if (toStringValue(cfg.tokenId).trim() && toStringValue(cfg.triggerCondition).trim()) {
      outcomeConditionRows.push({
        id: createId('oc'),
        tokenId: toStringValue(cfg.tokenId),
        outcomeLabel: toStringValue(cfg.outcomeLabel),
        triggerCondition: toStringValue(cfg.triggerCondition),
        triggerPriceCent: toStringValue(cfg.triggerPriceCent),
        maxPriceCent: toCentStringValue(cfg.maxPriceCent, cfg.maxPrice),
      });
    }
  }
  if (nodeType === 'trigger.position_drawdown') {
    fields.tokenId = toStringValue(cfg.tokenId).trim();
    if (!fields.entryPriceCent?.trim()) {
      const legacyEntry = Number(toStringValue(cfg.entryPrice).trim());
      if (Number.isFinite(legacyEntry) && legacyEntry > 0) {
        fields.entryPriceCent = String(legacyEntry * 100);
      }
    }
    if (Array.isArray(cfg.lossRules)) {
      for (const item of cfg.lossRules as Record<string, unknown>[]) {
        if (!isRecord(item)) continue;
        const lossPctRaw = toStringValue(item.lossPct).trim();
        const directionRaw = toStringValue(item.direction).trim().toLowerCase();
        const direction: 'down' | 'up' = directionRaw === 'up' ? 'up' : 'down';
        const windowMsValue = Number(toStringValue(item.windowMs).trim());
        const durationValue =
          Number.isFinite(windowMsValue) && windowMsValue > 0 ? String(Math.floor(windowMsValue)) : '';
        drawdownRuleRows.push({
          id: createId('dr'),
          direction,
          lossPct: lossPctRaw,
          durationValue,
        });
      }
    }
    if (drawdownRuleRows.length === 0) {
      const fallbackLossPct = toStringValue(cfg.lossPct).trim();
      const fallbackWindowMs = Number(toStringValue(cfg.windowMs).trim());
      const durationValue =
        Number.isFinite(fallbackWindowMs) && fallbackWindowMs > 0 ? String(Math.floor(fallbackWindowMs)) : '';
      if (fallbackLossPct) {
        drawdownRuleRows.push({
          id: createId('dr'),
          direction: 'down',
          lossPct: fallbackLossPct,
          durationValue,
        });
      }
    }
    if (drawdownRuleRows.length === 0) {
      drawdownRuleRows = [createEmptyDrawdownRuleRow()];
    }
  }

  const expression = parseExpressionDraft(cfg.expression);
  const patchRows = objectToRows(cfg.statePatch ?? cfg.state);

  return {
    fields,
    triggerSizeRows,
    outcomeConditionRows,
    drawdownRuleRows,
    expressionRows: expression.rows,
    expressionJoin: expression.join,
    expressionSupported: expression.supported,
    nestedExprMode: false,
    nestedExprGroup: null,
    statePatchRows: patchRows.length > 0 ? patchRows : [createEmptyKeyValueDraft()],
    advancedJson: safeJsonStringify(cfg),
  };
}

export function buildNodeConfigFromForm(
  nodeType: string,
  form: NodeConfigFormState
): Record<string, unknown> {
  const config: Record<string, unknown> = {};

  for (const field of NODE_FIELD_SCHEMAS[nodeType] || []) {
    const raw = (form.fields[field.key] ?? '').trim();
    if (!raw) continue;

    if (field.input === 'datetime-local') {
      const parsed = new Date(raw);
      config[field.key] = Number.isNaN(parsed.getTime()) ? raw : parsed.toISOString();
      continue;
    }

    if (NUMERIC_KEYS.has(field.key)) {
      const parsed = Number(raw);
      if (Number.isFinite(parsed)) {
        config[field.key] = parsed;
      }
      continue;
    }
    if (BOOLEAN_KEYS.has(field.key)) {
      const normalized = raw.toLowerCase();
      if (['true', '1', 'yes', 'y', 'on'].includes(normalized)) {
        config[field.key] = true;
        continue;
      }
      if (['false', '0', 'no', 'n', 'off'].includes(normalized)) {
        config[field.key] = false;
        continue;
      }
    }
    config[field.key] = raw;
  }

  if (nodeType === 'action.place_order') {
    const presetKindRaw = (form.fields.presetKind ?? '').trim();
    if (presetKindRaw) {
      config.presetKind = presetKindRaw;
    }

    const executionModeRaw = (form.fields.executionMode ?? '').trim().toLowerCase();
    if (executionModeRaw === 'market' || executionModeRaw === 'limit') {
      config.executionMode = executionModeRaw;
    } else {
      delete config.executionMode;
    }

    const sizeModeRaw = (form.fields.sizeMode ?? '').trim().toLowerCase();
    const sizeMode = sizeModeRaw === 'pct' ? 'pct' : 'usdc';
    config.sizeMode = sizeMode;

    if (sizeMode === 'pct') {
      delete config.sizeUsdc;
      delete config.targetNotionalUsdc;
    } else {
      delete config.sizePct;
    }

    const parsedMaxTriggers = Number(form.fields.maxTriggers ?? '');
    const normalizedMaxTriggers =
      Number.isFinite(parsedMaxTriggers) && parsedMaxTriggers > 0
        ? Math.min(20, Math.floor(parsedMaxTriggers))
        : null;
    const triggerSizes = (form.triggerSizeRows || [])
      .map((value) => Number(value.trim()))
      .filter((value) => Number.isFinite(value) && value > 0);
    if ((normalizedMaxTriggers ?? 0) > 1 && triggerSizes.length > 0) {
      const normalizedTriggerSizes = triggerSizes.slice(0, normalizedMaxTriggers ?? triggerSizes.length);
      config.triggerSizes = normalizedTriggerSizes;
      const firstValue = normalizedTriggerSizes[0];
      if (sizeMode === 'pct' && config.sizePct == null) {
        config.sizePct = firstValue;
      }
      if (sizeMode !== 'pct' && config.sizeUsdc == null && config.targetNotionalUsdc == null) {
        config.sizeUsdc = firstValue;
      }
    } else {
      delete config.triggerSizes;
    }

    const isPresetPlaceOrder = isPresetPlaceOrderMarker(config.presetKind, config.refKey);
    if (isPresetPlaceOrder) {
      config.kind = 'immediate';
      delete config.triggerCondition;
      delete config.triggerPrice;
      delete config.triggerPriceCent;
      if (isPresetBuySellPlaceOrderMarker(config.presetKind, config.refKey)) {
        config.executionMode = 'market';
      }
    }

    const sideRaw = toStringValue(config.side).trim().toLowerCase();
    const isBuySide = sideRaw === 'buy';
    const tpEnabled = config.tpEnabled === true;
    const slEnabled = config.slEnabled === true;
    if (!isBuySide) {
      delete config.tpEnabled;
      delete config.tpPriceCent;
      delete config.tpPrice;
      delete config.slEnabled;
      delete config.slPriceCent;
      delete config.slPrice;
    } else {
      if (!tpEnabled) {
        delete config.tpEnabled;
        delete config.tpPriceCent;
        delete config.tpPrice;
      }
      if (!slEnabled) {
        delete config.slEnabled;
        delete config.slPriceCent;
        delete config.slPrice;
      }
    }
  }

  if (nodeType === 'action.resolve_market') {
    const derivedScope = toResolveMarketScope(config.asset, config.timeframe);
    if (derivedScope) {
      config.marketScope = derivedScope;
    } else {
      delete config.marketScope;
    }
    delete config.slugPrefix;
  }
  if (nodeType === 'action.dual_dca') {
    const assetRaw =
      toStringValue(config.asset).trim().toLowerCase() ||
      toStringValue(config.coin).trim().toLowerCase();
    if (assetRaw) {
      config.asset = assetRaw;
      config.coin = assetRaw.toUpperCase();
    } else {
      delete config.asset;
      delete config.coin;
    }

    const timeframeRaw =
      toStringValue(config.timeframe).trim().toLowerCase() ||
      toStringValue(config.marketPeriod).trim().toLowerCase();
    const timeframe =
      timeframeRaw === '5' || timeframeRaw === '5min' || timeframeRaw === '5 min'
        ? '5m'
        : timeframeRaw === '15' || timeframeRaw === '15min' || timeframeRaw === '15 min'
          ? '15m'
          : timeframeRaw;
    if (timeframe) {
      config.timeframe = timeframe;
      config.marketPeriod = timeframe;
    } else {
      delete config.timeframe;
      delete config.marketPeriod;
    }

    const sideModeRaw =
      toStringValue(config.sideMode).trim().toLowerCase() ||
      toStringValue(config.side).trim().toLowerCase();
    if (sideModeRaw) {
      const sideMode =
        sideModeRaw === 'up' || sideModeRaw === 'down' || sideModeRaw === 'all'
          ? sideModeRaw
          : sideModeRaw;
      config.sideMode = sideMode;
      config.side = sideMode;
    } else {
      delete config.sideMode;
      delete config.side;
    }

    const baseSizingRaw =
      toStringValue(config.baseSizing).trim().toLowerCase() ||
      toStringValue(config.baseSizeMode).trim().toLowerCase();
    if (baseSizingRaw) {
      const baseSizing =
        baseSizingRaw === 'usdc' || baseSizingRaw === 'shares'
          ? baseSizingRaw
          : baseSizingRaw;
      config.baseSizing = baseSizing;
      config.baseSizeMode = baseSizing;
      if (baseSizing === 'shares') {
        delete config.baseUsdc;
      } else if (baseSizing === 'usdc') {
        delete config.baseShares;
      }
    } else {
      delete config.baseSizing;
      delete config.baseSizeMode;
    }

    const derivedScope = toResolveMarketScope(config.asset, config.timeframe);
    if (derivedScope) {
      config.marketScope = derivedScope;
    } else {
      delete config.marketScope;
    }

    if (!toStringValue(config.refKey).trim()) {
      delete config.refKey;
    }
  }

  if (nodeType === 'trigger.market_price') {
    const marketModeRaw = toStringValue(form.fields.marketMode ?? config.marketMode).trim().toLowerCase();
    const marketMode = marketModeRaw === 'auto_scope' ? 'auto_scope' : 'fixed';
    config.marketMode = marketMode;
    const priceModeRaw = toStringValue(config.priceMode).trim().toLowerCase();
    const validPriceModes2 = ['midpoint', 'raw', 'best_bid', 'best_ask'];
    config.priceMode = validPriceModes2.includes(priceModeRaw) ? priceModeRaw : 'midpoint';

    const repeatModeRaw = toStringValue(config.repeatMode).trim().toLowerCase();
    config.repeatMode = repeatModeRaw === 'once' ? 'once' : 'loop';

    const onceScopeRaw = toStringValue(config.onceScope).trim().toLowerCase();
    if (config.repeatMode === 'once') {
      if (onceScopeRaw === 'market' || onceScopeRaw === 'run') {
        config.onceScope = onceScopeRaw;
      } else {
        config.onceScope = marketMode === 'auto_scope' ? 'market' : 'run';
      }
      config.onceScopeVersion = TRIGGER_MARKET_ONCE_SCOPE_VERSION;
    }

    const selectionRaw = toStringValue(config.marketSelection).trim().toLowerCase();
    config.marketSelection = selectionRaw || 'latest_by_slug';

    const confirmationMsRaw = toStringValue(form.fields.confirmationMs).trim();
    if (confirmationMsRaw) {
      const parsedConfirmationMs = Number(confirmationMsRaw);
      if (Number.isInteger(parsedConfirmationMs) && parsedConfirmationMs >= 0) {
        config.confirmationMs = parsedConfirmationMs;
      } else {
        delete config.confirmationMs;
      }
    }

    const scopeRaw = toStringValue(config.marketScope).trim().toLowerCase();
    if (marketMode === 'auto_scope') {
      if (scopeRaw && RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[scopeRaw]) {
        config.marketScope = scopeRaw;
      } else {
        delete config.marketScope;
      }
      const protectionModeRaw = toStringValue(config.protectionMode).trim().toLowerCase();
      if (protectionModeRaw === 'underlying_confirm') {
        config.protectionMode = 'underlying_confirm';
        const protectionPresetRaw = toStringValue(config.protectionPreset).trim().toLowerCase();
        config.protectionPreset =
          protectionPresetRaw === 'loose' ||
          protectionPresetRaw === 'balanced' ||
          protectionPresetRaw === 'strict'
            ? protectionPresetRaw
            : 'balanced';
      } else {
        delete config.protectionMode;
        delete config.protectionPreset;
      }
      // auto_scope resolves market slug at runtime.
      delete config.marketSlug;
      // Cycle window focus
      const cycleWindowModeRaw2 = toStringValue(config.cycleWindowMode).trim().toLowerCase();
      if (cycleWindowModeRaw2 === 'first' || cycleWindowModeRaw2 === 'last') {
        config.cycleWindowMode = cycleWindowModeRaw2;
        const cwSecsRaw = Number(toStringValue(config.cycleWindowSecs).trim());
        if (Number.isInteger(cwSecsRaw) && cwSecsRaw > 0) {
          config.cycleWindowSecs = cwSecsRaw;
        } else {
          delete config.cycleWindowMode;
          delete config.cycleWindowSecs;
        }
      } else {
        delete config.cycleWindowMode;
        delete config.cycleWindowSecs;
      }
    } else {
      delete config.marketScope;
      delete config.marketSelection;
      delete config.protectionMode;
      delete config.protectionPreset;
      delete config.cycleWindowMode;
      delete config.cycleWindowSecs;
    }

    if (config.repeatMode !== 'once') {
      delete config.onceScope;
      delete config.onceScopeVersion;
    }
  }

  if (nodeType === 'trigger.position_drawdown') {
    const combineModeRaw = toStringValue(config.combineMode).trim().toLowerCase();
    if (combineModeRaw === 'and' || combineModeRaw === 'or') {
      config.combineMode = combineModeRaw;
    } else {
      delete config.combineMode;
    }
    const tokenIdRaw = toStringValue(form.fields.tokenId).trim();
    if (tokenIdRaw) {
      config.tokenId = tokenIdRaw;
    } else {
      delete config.tokenId;
    }

    const entryPriceCentRaw = Number(form.fields.entryPriceCent?.trim() ?? '');
    if (Number.isFinite(entryPriceCentRaw) && entryPriceCentRaw > 0 && entryPriceCentRaw <= 100) {
      config.entryPriceCent = entryPriceCentRaw;
    } else {
      delete config.entryPriceCent;
    }

    const rules = (form.drawdownRuleRows || [])
      .map((row) => {
        const lossPct = Number(row.lossPct.trim());
        if (!Number.isFinite(lossPct) || lossPct <= 0 || lossPct > 100) return null;
        const direction = row.direction === 'up' ? 'up' : 'down';

        const durationRaw = row.durationValue.trim();
        let windowMs: number | undefined;
        if (durationRaw) {
          const durationValue = Number(durationRaw);
          if (!Number.isFinite(durationValue) || durationValue <= 0) return null;
          windowMs = Math.floor(durationValue);
          if (!Number.isFinite(windowMs) || windowMs <= 0) return null;
        }

        const item: Record<string, unknown> = { lossPct, direction };
        if (windowMs != null) item.windowMs = windowMs;
        return item;
      })
      .filter((item): item is Record<string, unknown> => item != null);

    if (rules.length > 0) {
      config.lossRules = rules;
    } else {
      delete config.lossRules;
    }
    delete config.sourceTradeId;
    delete config.entryPrice;
    delete config.lossPct;
    delete config.windowSec;
    delete config.windowMs;
  }

  if (nodeType === 'trigger.open_positions') {
    const maxPriceCentRaw = Number(form.fields.maxPriceCent?.trim() ?? '');
    if (Number.isFinite(maxPriceCentRaw) && maxPriceCentRaw > 0 && maxPriceCentRaw <= 100) {
      config.maxPriceCent = maxPriceCentRaw;
    } else {
      delete config.maxPriceCent;
    }
    delete config.maxPrice;
  }

  if ((nodeType === 'trigger.open_positions' || nodeType === 'trigger.market_price') && form.outcomeConditionRows.length > 0) {
    const conditions = form.outcomeConditionRows
      .filter((row) => {
        const tokenId = row.tokenId.trim();
        const outcomeLabel = row.outcomeLabel.trim();
        const triggerCondition = row.triggerCondition.trim();
        const triggerPriceCent = Number(row.triggerPriceCent.trim());
        const maxPriceCentRaw = row.maxPriceCent.trim();
        const maxPriceCent = maxPriceCentRaw ? Number(maxPriceCentRaw) : null;
        const hasValidMaxPriceCent =
          !maxPriceCentRaw ||
          (Number.isFinite(maxPriceCent) && (maxPriceCent as number) > 0 && (maxPriceCent as number) <= 100);
        if (!tokenId || !outcomeLabel) return false;
        if (triggerCondition !== 'cross_above' && triggerCondition !== 'cross_below') return false;
        return Number.isFinite(triggerPriceCent) && triggerPriceCent > 0 && triggerPriceCent <= 100 && hasValidMaxPriceCent;
      })
      .map((row) => {
        const priceCent = Number(row.triggerPriceCent.trim());
        const maxPriceCentRaw = row.maxPriceCent.trim();
        const maxPriceCent = maxPriceCentRaw ? Number(maxPriceCentRaw) : null;
        const condition: Record<string, unknown> = {
          tokenId: row.tokenId.trim(),
          outcomeLabel: row.outcomeLabel.trim(),
          triggerCondition: row.triggerCondition.trim(),
          triggerPriceCent: Number.isFinite(priceCent) ? priceCent : 0,
        };
        if (
          maxPriceCentRaw &&
          Number.isFinite(maxPriceCent) &&
          (maxPriceCent as number) > 0 &&
          (maxPriceCent as number) <= 100
        ) {
          condition.maxPriceCent = maxPriceCent;
        }
        return condition;
      });
    if (conditions.length > 0) {
      config.outcomeConditions = conditions;
      delete config.tokenId;
      delete config.triggerCondition;
      delete config.triggerPriceCent;
      delete config.maxPriceCent;
      delete config.maxPrice;
    }
  }

  if (nodeType === 'logic.if' || nodeType === 'logic.switch') {
    if (form.nestedExprMode && form.nestedExprGroup) {
      config.expression = nestedExprGroupToJsonLogic(form.nestedExprGroup);
    } else {
      config.expression = buildExpression(form.expressionRows, form.expressionJoin);
    }
  }

  if (nodeType === 'action.set_state') {
    config.statePatch = buildObjectFromKeyValueDrafts(form.statePatchRows);
  }

  if (nodeType === 'action.telegram_notify') {
    delete config.botToken;
  }

  return config;
}

export function parseEdgeConditionToForm(condition: unknown): EdgeConditionFormState {
  if (condition == null) {
    return {
      enabled: false,
      conditionRow: createEmptyConditionDraft(),
      conditionSupported: true,
      advancedJson: '',
    };
  }

  const parsed = parseSimpleCondition(condition);
  if (parsed) {
    return {
      enabled: true,
      conditionRow: { id: createId('edge_cond'), ...parsed },
      conditionSupported: true,
      advancedJson: safeJsonStringify(condition),
    };
  }

  return {
    enabled: true,
    conditionRow: createEmptyConditionDraft(),
    conditionSupported: false,
    advancedJson: safeJsonStringify(condition),
  };
}

export function buildEdgeConditionFromForm(form: EdgeConditionFormState): Record<string, unknown> | null {
  if (!form.enabled) return null;
  if (!form.conditionRow.leftVar.trim()) return null;
  return buildSimpleCondition(form.conditionRow);
}

type ExpressionNode = import('@/lib/types').ExpressionLeaf | import('@/lib/types').ExpressionGroup;

function leafToJsonLogic(leaf: import('@/lib/types').ExpressionLeaf): Record<string, unknown> {
  const leftOp = { var: leaf.leftVar || 'market_price' };

  if (leaf.operator === 'between') {
    const parts = String(leaf.rightValue).split(',').map((s) => Number(s.trim()));
    const lo = Number.isFinite(parts[0]) ? parts[0] : 0;
    const hi = Number.isFinite(parts[1]) ? parts[1] : 100;
    return { '<=': [lo, leftOp, hi] };
  }
  if (leaf.operator === 'in') {
    const items = String(leaf.rightValue).split(',').map((s) => s.trim());
    return { in: [leftOp, items] };
  }
  if (leaf.operator === 'contains') {
    return { in: [leaf.rightValue, leftOp] };
  }

  const rightVal = leaf.rightType === 'number'
    ? (Number.isFinite(Number(leaf.rightValue)) ? Number(leaf.rightValue) : 0)
    : leaf.rightType === 'boolean'
      ? String(leaf.rightValue).trim().toLowerCase() === 'true'
      : leaf.rightValue;

  return { [leaf.operator]: [leftOp, rightVal] };
}

export function nestedExprGroupToJsonLogic(group: import('@/lib/types').ExpressionGroup): Record<string, unknown> {
  if (group.children.length === 0) return { '==': [1, 1] };
  const mapped = group.children.map((child: ExpressionNode) => {
    if (child.type === 'leaf') return leafToJsonLogic(child);
    return nestedExprGroupToJsonLogic(child);
  });
  if (mapped.length === 1) return mapped[0];
  return { [group.operator]: mapped };
}

function tryParseJsonLogicLeaf(obj: Record<string, unknown>): import('@/lib/types').ExpressionLeaf | null {
  for (const op of ['>', '>=', '<', '<=', '==', '!='] as const) {
    if (!Array.isArray(obj[op]) || (obj[op] as unknown[]).length !== 2) continue;
    const [left, right] = obj[op] as [unknown, unknown];
    if (!left || typeof left !== 'object' || !('var' in (left as Record<string, unknown>))) continue;
    const leftVar = String((left as Record<string, unknown>).var);
    const rightType = typeof right === 'number' ? 'number' : typeof right === 'boolean' ? 'boolean' : 'string';
    return { type: 'leaf', leftVar, operator: op, rightValue: right, rightType };
  }
  if (Array.isArray(obj.in) && (obj.in as unknown[]).length === 2) {
    const [a, b] = obj.in as [unknown, unknown];
    if (a && typeof a === 'object' && 'var' in (a as Record<string, unknown>)) {
      return { type: 'leaf', leftVar: String((a as Record<string, unknown>).var), operator: 'in', rightValue: Array.isArray(b) ? (b as unknown[]).join(', ') : String(b), rightType: 'string' };
    }
    if (b && typeof b === 'object' && 'var' in (b as Record<string, unknown>)) {
      return { type: 'leaf', leftVar: String((b as Record<string, unknown>).var), operator: 'contains', rightValue: String(a), rightType: 'string' };
    }
  }
  return null;
}

function parseJsonLogicChild(item: unknown): ExpressionNode | null {
  if (!item || typeof item !== 'object' || Array.isArray(item)) return null;
  const obj = item as Record<string, unknown>;
  if (Array.isArray(obj.and)) {
    const children = (obj.and as unknown[]).map(parseJsonLogicChild).filter(Boolean) as ExpressionNode[];
    if (children.length > 0) return { type: 'group', operator: 'and', children };
  }
  if (Array.isArray(obj.or)) {
    const children = (obj.or as unknown[]).map(parseJsonLogicChild).filter(Boolean) as ExpressionNode[];
    if (children.length > 0) return { type: 'group', operator: 'or', children };
  }
  return tryParseJsonLogicLeaf(obj);
}

export function jsonLogicToNestedExprGroup(logic: unknown): import('@/lib/types').ExpressionGroup | null {
  if (!logic || typeof logic !== 'object' || Array.isArray(logic)) return null;
  const obj = logic as Record<string, unknown>;
  if (Array.isArray(obj.and)) {
    const children = (obj.and as unknown[]).map(parseJsonLogicChild).filter(Boolean) as ExpressionNode[];
    if (children.length > 0) return { type: 'group', operator: 'and', children };
  }
  if (Array.isArray(obj.or)) {
    const children = (obj.or as unknown[]).map(parseJsonLogicChild).filter(Boolean) as ExpressionNode[];
    if (children.length > 0) return { type: 'group', operator: 'or', children };
  }
  const leaf = tryParseJsonLogicLeaf(obj);
  if (leaf) return { type: 'group', operator: 'and', children: [leaf] };
  return null;
}

const CONTEXT_BASE_KEYS = new Set([
  'sourceTradeId',
  'marketSlug',
  'tokenId',
  'outcomeLabel',
  'autoClaimEnabled',
]);

export function parseContextToForm(context: unknown): ContextFormState {
  const ctx = isRecord(context) ? context : {};
  const sourceTradeId = toStringValue(ctx.sourceTradeId);
  const marketSlug = toStringValue(ctx.marketSlug);
  const tokenId = toStringValue(ctx.tokenId);
  const outcomeLabel = toStringValue(ctx.outcomeLabel);
  const autoClaimEnabled = toBooleanValue(ctx.autoClaimEnabled);

  const extras: KeyValueDraft[] = [];
  for (const [key, value] of Object.entries(ctx)) {
    if (CONTEXT_BASE_KEYS.has(key)) continue;
    extras.push({
      id: createId('ctx'),
      key,
      value: toStringValue(value),
      valueType: valueTypeOf(value),
    });
  }

  return {
    sourceTradeId,
    marketSlug,
    tokenId,
    outcomeLabel,
    autoClaimEnabled,
    extras,
    advancedJson: safeJsonStringify(ctx),
  };
}

export function buildContextFromForm(form: ContextFormState): Record<string, unknown> {
  const context: Record<string, unknown> = {};

  const sourceTradeId = form.sourceTradeId.trim();
  if (sourceTradeId) {
    const parsed = Number(sourceTradeId);
    if (Number.isFinite(parsed)) context.sourceTradeId = parsed;
  }

  if (form.marketSlug.trim()) context.marketSlug = form.marketSlug.trim();
  if (form.tokenId.trim()) context.tokenId = form.tokenId.trim();
  if (form.outcomeLabel.trim()) context.outcomeLabel = form.outcomeLabel.trim();
  if (form.autoClaimEnabled) context.autoClaimEnabled = true;

  const extraValues = buildObjectFromKeyValueDrafts(form.extras);
  for (const [key, value] of Object.entries(extraValues)) {
    context[key] = value;
  }

  return context;
}
