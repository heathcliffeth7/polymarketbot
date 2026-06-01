import type { NodeFieldSchema } from './types';
import { PTB_MODE_OPTIONS } from './ptb-modes';
import { ACTION_PLACE_ORDER_FIELD_SCHEMA } from './action-place-order-schema';

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
    {
      key: 'bindingMode',
      label: 'Aksiyon Binding Modu',
      input: 'select',
      options: [
        { label: 'Standard', value: 'standard' },
        { label: 'Pair Lock', value: 'pair_lock_only' },
        { label: 'DCA Live', value: 'dca_live_only' },
        { label: 'Positive Flip Grid', value: 'positive_quantity_flip_grid_only' },
        { label: 'RevengeFlip', value: 'revenge_flip_only' },
      ],
      help: 'Pair Lock icin downstream mode=pair_lock; DCA Live icin mode=dca_live_v1; Positive Flip Grid icin grid modlari; RevengeFlip icin mode=revenge_flip_v1 gerekir. Ek olarak action.notify/action.telegram_notify baglanabilir.',
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
        { label: 'Ozel Aralik (custom_range)', value: 'custom_range' },
      ],
      help: 'Cycle icinde sadece belirli bir zaman penceresinde tetik degerlendirilir. Auto-scope + `last` modunda son pencereye girdiginde ilk gorulen uygun fiyat da tetikleyebilir; fixed markette gercek cross aranir.',
    },
    {
      key: 'cycleWindowSecs',
      label: 'Pencere Süresi (saniye)',
      input: 'number',
      help: 'Kac saniye boyunca tetik degerlendirilecek. Or: 5m cycle, last 60 -> son 60sn. Auto-scope + `last` modunda son pencereye in-zone giris kabul edilir; fixed markette pencere icinde gercek gecis aranir.',
    },
    {
      key: 'cycleWindowStartSec',
      label: 'Pencere Baslangic (saniye)',
      input: 'number',
      help: 'Cycle baslangicindan itibaren kac saniye sonra pencere acilir. Ornek: 5m cycle, 40 -> cycle basladigindan 40sn sonra. Cycle suresi: 5m=300sn, 15m=900sn.',
    },
    {
      key: 'cycleWindowEndSec',
      label: 'Pencere Bitis (saniye)',
      input: 'number',
      help: 'Cycle baslangicindan itibaren kac saniye sonra pencere kapanir. startSec < endSec olmali. Ornek: 5m cycle, 300 -> cycle sonuna kadar. Cycle suresi: 5m=300sn, 15m=900sn.',
    },
    {
      key: 'autoSellOnWindowEnd',
      label: 'Pencere Bitince Pozisyonu Kapat',
      input: 'checkbox',
      help: 'Aktifken, custom_range penceresi bittiginde acik pozisyon varsa otomatik olarak market emriyle satar. Stop-loss mantigi ile calisir.',
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
      key: 'priceToBeatTriggerEnabled',
      label: 'Price-to-Beat Gate',
      input: 'checkbox',
      help: 'Aktifken tetik price-to-beat farki araligina gore degerlendirilir. Fiyat kosulu bos birakilabilir. Sadece auto_scope.',
    },
    {
      key: 'priceToBeatMode',
      label: 'PTB Modu',
      input: 'select',
      options: PTB_MODE_OPTIONS,
      help: 'Manual modda fark alanlari kullanilir. Auto modlarda tamamlanmis market excursion, volatility yuzdesi, signal formula veya IV mismatch edge kullanilir.',
    },
    {
      key: 'priceToBeatTriggerUnit',
      label: 'Fark Birimi',
      input: 'select',
      options: [
        { label: 'USD', value: 'usd' },
        { label: 'Cent', value: 'cent' },
      ],
      help: 'USD: dolar cinsinden. Cent: sent cinsinden. Ornek: cent modunda 1 = $0.01.',
    },
    {
      key: 'priceToBeatTriggerMinGap',
      label: 'Minimum Fark',
      input: 'number',
      help: 'Price-to-beat farki bu degerin altindaysa tetik ateslenmez.',
    },
    {
      key: 'priceToBeatTriggerMaxGap',
      label: 'Tavan Fark',
      input: 'number',
      help: 'Price-to-beat farki bu degerin ustundeyse tetik ateslenmez. Opsiyonel.',
    },
    {
      key: 'priceMode',
      label: 'Fiyat Kaynağı',
      input: 'select',
      options: [
        { label: 'composite (önerilen)', value: 'composite' },
        { label: 'site display (Polymarket UI)', value: 'site_display' },
        { label: 'midpoint', value: 'midpoint' },
        { label: 'raw trade (fallbacklı)', value: 'raw' },
        { label: 'last trade (strict)', value: 'last_trade' },
        { label: 'best bid (satış için)', value: 'best_bid' },
        { label: 'best ask (alış için)', value: 'best_ask' },
      ],
      help: 'composite: cross_above/level_above icin best_bid ile last_trade fiyatinin yuksek olanini, cross_below/level_below icin dusuk olanini kullanir. site_display: Polymarket UI fiyati; spread 10c altindaysa midpoint, ustundeyse son islem fiyati. midpoint: best bid/ask ortalamasi. raw: son islem fiyatini kullanir, yoksa midpoint fallback yapar. last_trade: sadece son islem fiyatini kullanir, fallback yapmaz. best_bid: en iyi alim fiyati. best_ask: en iyi satim fiyati.',
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
        { label: 'level_above', value: 'level_above' },
        { label: 'level_below', value: 'level_below' },
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
      help: 'Opsiyonel. cross_above/level_above ile birlikte kullanirsan triggerPrice ile bu tavan arasina girildiginde tetik sayilir; tavanin ustu tetiklenmez.',
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
  'action.place_order': ACTION_PLACE_ORDER_FIELD_SCHEMA,
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
