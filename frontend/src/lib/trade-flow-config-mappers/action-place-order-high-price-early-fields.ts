import type { NodeFieldSchema } from './types';

export const ACTION_PLACE_ORDER_HIGH_PRICE_EARLY_FIELDS: NodeFieldSchema[] = [
  {
    key: 'priceToBeatIvHighPriceEarlyReversalGuardEnabled',
    label: 'IV High Price Early Guard',
    input: 'checkbox',
    help: 'Pahali ve erken entry icin q-saturation/reversal risk guardini acar.',
  },
  {
    key: 'priceToBeatIvHighPriceEarlyRefCent',
    label: 'IV High Early Ref',
    input: 'number',
    help: 'Guard icin decision ref cent esigi.',
  },
  {
    key: 'priceToBeatIvHighPriceEarlyRemainingSec',
    label: 'IV High Early Sec',
    input: 'number',
    help: 'Guardin erken entry saymasi icin minimum kalan saniye.',
  },
  {
    key: 'priceToBeatIvHighPriceEarlyMaxStaleMs',
    label: 'IV High Early Stale Ms',
    input: 'number',
    help: 'Bu Chainlink yasi ustunde high-price early gap add uygulanir.',
  },
  {
    key: 'priceToBeatIvHighPriceEarlyStaleGapAdd',
    label: 'IV High Early Stale Add',
    input: 'number',
    help: 'High-price early stale durumda minGapStrength uzerine eklenir.',
  },
  {
    key: 'priceToBeatIvHighPriceEarlyBinanceMissingGapAdd',
    label: 'IV High Early Binance Add',
    input: 'number',
    help: 'High-price early Binance q missing/fail-open durumunda minGapStrength uzerine eklenir.',
  },
  {
    key: 'priceToBeatIvHighPriceEarlyQExtremeCent',
    label: 'IV High Early Q Extreme',
    input: 'number',
    help: 'Q-saturation icin q_final cent esigi.',
  },
  {
    key: 'priceToBeatIvHighPriceEarlyQExtremeMinGapStrength',
    label: 'IV High Early Q Gap Min',
    input: 'number',
    help: 'Q-extreme aktifken effective minGapStrength floor degeri.',
  },
  {
    key: 'priceToBeatIvHighPriceEarlyQExtremeMaxStaleMs',
    label: 'IV High Early Q Stale Ms',
    input: 'number',
    help: 'Q-extreme aktifken izin verilen maksimum Chainlink yasi.',
  },
  {
    key: 'priceToBeatIvHighPriceEarlyQExtremeRequireBinanceQ',
    label: 'IV High Early Req Binance Q',
    input: 'checkbox',
    help: 'Q-extreme aktifken q_binance yoksa fail-closed bloklar.',
  },
  {
    key: 'priceToBeatIvHighPriceEarlyQExtremeRequireCleanStrongCex',
    label: 'IV High Early Req CEX',
    input: 'checkbox',
    help: 'Q-extreme aktifken CEX consensus strong ve clean degilse bloklar.',
  },
];
