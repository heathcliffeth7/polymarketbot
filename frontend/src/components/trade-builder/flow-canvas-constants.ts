import type { ReactNode } from 'react';
import type { Edge, Node } from '@xyflow/react';
import type {
  NodeExecutionStatus,
  TradeFlowOpenPositionOption,
  TradeFlowOpenPositionsMeta,
} from '@/lib/types';

export type FlowNodeData = {
  nodeType: string;
  config: Record<string, unknown>;
  executionStatus?: NodeExecutionStatus;
  livePrice?: number | null;
  groupId?: string;
  groupColor?: string;
};

export interface NodeGroup {
  id: string;
  name: string;
  color: string;
}

export const GROUP_COLORS = [
  { label: 'Mavi', value: '#dbeafe', border: '#93c5fd' },
  { label: 'Yesil', value: '#dcfce7', border: '#86efac' },
  { label: 'Mor', value: '#f3e8ff', border: '#c084fc' },
  { label: 'Turuncu', value: '#ffedd5', border: '#fdba74' },
  { label: 'Pembe', value: '#fce7f3', border: '#f9a8d4' },
];

export type FlowEdgeData = {
  edgeType: string;
  condition: Record<string, unknown> | null;
};

export type FlowNode = Node<FlowNodeData, 'flowNode'>;
export type FlowEdge = Edge<FlowEdgeData, 'smoothstep'>;
export type PlaceOrderPresetKind = 'sell_current_position' | 'buy_current_position' | 'place_order';

export type PlaceOrderPresetSeed = {
  sourceTradeId: number | null;
  marketSlug: string;
  tokenId: string;
  outcomeLabel: string;
};

export type NodePaletteCategory = 'all' | 'trigger' | 'logic' | 'action';

export interface FlowCanvasEditorProps {
  graph: import('@/lib/types').TradeFlowGraph;
  onGraphChange: (nextGraph: import('@/lib/types').TradeFlowGraph) => void;
  onError: (message: string | null) => void;
  onPendingNodeDraftChange?: (hasPending: boolean) => void;
  openPositions: TradeFlowOpenPositionOption[];
  openPositionsMeta: TradeFlowOpenPositionsMeta | null;
  openPositionsLoading: boolean;
  onApplyContextPatch: (patch: Record<string, unknown>) => void;
  leftPanelTopSlot?: ReactNode;
  executionStates?: import('@/lib/types').NodeExecutionState[];
  livePrices?: Record<string, number>;
  globalTelegramBotTokenMasked?: string | null;
  globalTelegramChatId?: string | null;
}

export const NODE_TYPE_OPTIONS: Array<{ value: string; label: string }> = [
  { value: 'trigger.market_price', label: 'Tetik: Piyasa Fiyati' },
  { value: 'trigger.sell_progress', label: 'Tetik: Satis Ilerlemesi' },
  { value: 'trigger.open_positions', label: 'Tetik: Mevcut Pozisyonlar' },
  { value: 'trigger.position_drawdown', label: 'Tetik: Pozisyon Dusus (Drawdown)' },
  { value: 'trigger.time_window', label: 'Tetik: Zaman Penceresi' },
  { value: 'logic.if', label: 'Mantik: If / Else' },
  { value: 'logic.switch', label: 'Mantik: Switch' },
  { value: 'logic.delay', label: 'Mantik: Gecikme' },
  { value: 'logic.retry', label: 'Mantik: Retry' },
  { value: 'action.resolve_market', label: 'Aksiyon: Market Coz' },
  { value: 'action.dual_dca', label: 'Aksiyon: Cift Tarafli DCA' },
  { value: 'action.place_order', label: 'Aksiyon: Emir Gonder' },
  { value: 'action.cancel_order', label: 'Aksiyon: Emir Iptal' },
  { value: 'action.update_order', label: 'Aksiyon: Emir Guncelle' },
  { value: 'action.set_state', label: 'Aksiyon: Durum Guncelle' },
  { value: 'action.notify', label: 'Aksiyon: Bildirim' },
  { value: 'action.telegram_notify', label: 'Aksiyon: Telegram Bildirim' },
];

export const EDGE_TYPE_OPTIONS: Array<{ value: string; label: string }> = [
  { value: 'default', label: 'default' },
  { value: 'on_success', label: 'on_success' },
  { value: 'on_error', label: 'on_error' },
  { value: 'on_true', label: 'on_true' },
  { value: 'on_false', label: 'on_false' },
  { value: 'on_retry', label: 'on_retry' },
  { value: 'case:yes', label: 'case:yes' },
  { value: 'case:no', label: 'case:no' },
];

export const NODE_TYPE_LABEL = new Map(NODE_TYPE_OPTIONS.map((item) => [item.value, item.label]));

export const EDGE_STROKE_COLOR = '#64748b';
export const EDGE_LABEL_COLOR = '#334155';
export const EDGE_LABEL_BG_COLOR = '#e2e8f0';

export const EDGE_LABEL_COLORS: Record<string, { stroke: string; bg: string; text: string }> = {
  on_success: { stroke: '#22c55e', bg: '#dcfce7', text: '#166534' },
  on_error: { stroke: '#ef4444', bg: '#fee2e2', text: '#991b1b' },
  on_true: { stroke: '#3b82f6', bg: '#dbeafe', text: '#1e40af' },
  on_false: { stroke: '#a855f7', bg: '#f3e8ff', text: '#6b21a8' },
  on_retry: { stroke: '#f59e0b', bg: '#fef3c7', text: '#92400e' },
};

export const NODE_PALETTE_CATEGORIES: Array<{ value: NodePaletteCategory; label: string }> = [
  { value: 'all', label: 'Tumu' },
  { value: 'trigger', label: 'Trigger' },
  { value: 'logic', label: 'Logic' },
  { value: 'action', label: 'Action' },
];

export interface NodeHelpFieldTip {
  field: string;
  description: string;
}

export interface NodeHelpExample {
  title: string;
  summary: string;
  values: Array<{ key: string; value: string }>;
  flowSteps: string[];
  expectedOutcome?: string;
}

export interface NodeHelpContent {
  title: string;
  quickExplain: string;
  simpleExamples: Array<{
    title: string;
    lines: string[];
  }>;
  fieldTips: NodeHelpFieldTip[];
  examples: NodeHelpExample[];
  checklist: string[];
}

export interface NodeFieldHelpItem {
  title: string;
  description: string;
  example?: string;
  effect?: string;
  whatHappensIfLowHigh?: string[];
  simpleExamples?: string[];
  tips?: string[];
}

export const NODE_HELP_CONTENT: Partial<Record<string, NodeHelpContent>> = {
  'action.dual_dca': {
    title: 'Dual DCA nasil calisir?',
    quickExplain:
      'Bu node, sectigin coin/periyottaki up/down marketlerine DCA plani kurar. Degerler publish oncesi doldurulmazsa flow dogrulama hatasi alirsin.',
    simpleExamples: [
      {
        title: 'Basit Baslangic (BTC 5m)',
        lines: [
          'Ayar: asset=btc, timeframe=5m, sideMode=all',
          'Ne olur: Sistem BTC 5m marketini bulur ve iki tarafi izler',
          'Ayar: baseSizing=shares, baseShares=10',
          'Ne olur: Tetikte 10 share bazli plan baslar',
        ],
      },
      {
        title: 'USDC Modu',
        lines: [
          'Ayar: asset=eth, timeframe=15m, sideMode=down',
          'Ne olur: Sadece DOWN tarafinda plan olusur',
          'Ayar: baseSizing=usdc, baseUsdc=8',
          'Ne olur: Her tetikte USDC bazli boyut kullanilir',
        ],
      },
      {
        title: 'Guvenli Test Degerleri',
        lines: [
          'Ayar: dcaLevels=2, nearStep=0.1, stepMult=1.1',
          'Ne olur: Base + 2 kademe (toplam 3 seviye) yakin-orta mesafede hesaplanir',
          'Ayar: sizeMult=1',
          'Ne olur: Kademe buyudukce miktar artmaz, sabit kalir',
        ],
      },
      {
        title: 'Risk Kapali Mod',
        lines: [
          'Ayar: tpProfitPct=0, slLossPct=0, slSpreadPct=0',
          'Ne olur: TP/SL kaynakli ek cikis kosulu uygulanmaz',
        ],
      },
    ],
    fieldTips: [
      { field: 'asset', description: 'Hangi coin icin market aranacagini belirler (btc/eth/sol/xrp).' },
      { field: 'timeframe', description: '5m veya 15m market periyodunu belirler.' },
      {
        field: 'sideMode',
        description:
          'up/down tek tarafli yon tahmini yapar; all iki tarafa birden merdiven kurar.',
      },
      { field: 'baseSizing', description: 'Temel boyutun shares mi usdc mi olacagini belirler.' },
      { field: 'dcaLevels', description: 'Base haric kac ek DCA kademesi olacagini belirler.' },
      { field: 'nearStep / stepMult', description: 'Kademeler arasi fiyat uzakligini hesaplar.' },
      { field: 'sizeMult', description: 'Her kademede miktar carpani olarak kullanilir.' },
      { field: 'tp/sl/spread', description: 'Risk esikleri USDC cinsindendir; 0 veya pozitif deger girilir.' },
    ],
    examples: [
      {
        title: 'Ornek 1 - BTC 5m (All, Shares)',
        summary: 'Iki tarafi da takip eden standart senaryo.',
        values: [
          { key: 'asset', value: 'btc' },
          { key: 'timeframe', value: '5m' },
          { key: 'sideMode', value: 'all' },
          { key: 'baseSizing', value: 'shares' },
          { key: 'baseShares', value: '10' },
          { key: 'dcaLevels', value: '2' },
          { key: 'nearStep / stepMult / sizeMult', value: '0.1 / 1.1 / 1' },
          { key: 'minPriceDistanceCent / cutoffMin', value: '1 / 3' },
          { key: 'tpProfitPct / slLossPct / slSpreadPct', value: '0 / 0 / 0' },
        ],
        flowSteps: [
          'Sistem aktif BTC 5m market slugini bulur.',
          'sideMode=all oldugu icin UP ve DOWN taraflarini birlikte izler.',
          'baseShares=10 ile baz plan olusur, dcaLevels=2 ile Base haric iki kademe eklenir.',
          'nearStep/stepMult kademe fiyatlarini, sizeMult kademe boyutlarini hesaplar.',
          'cutoffMin=3 oldugu icin market kapanisina 3 dakika kala yeni alim acmaz ve bekleyen conditional emirleri iptal ister.',
        ],
        expectedOutcome:
          'Tetik geldiginde iki tarafli DCA plani calisir; risk degerleri 0 oldugu icin TP/SL tetigi devreye girmez.',
      },
      {
        title: 'Ornek 2 - ETH 15m (Down, USDC)',
        summary: 'Sadece down tarafi icin USDC bazli senaryo.',
        values: [
          { key: 'asset', value: 'eth' },
          { key: 'timeframe', value: '15m' },
          { key: 'sideMode', value: 'down' },
          { key: 'baseSizing', value: 'usdc' },
          { key: 'baseUsdc', value: '8' },
          { key: 'dcaLevels', value: '3' },
          { key: 'nearStep / stepMult / sizeMult', value: '0.08 / 1.2 / 1' },
          { key: 'minPriceDistanceCent / cutoffMin', value: '1 / 4' },
          { key: 'tpProfitPct / slLossPct / slSpreadPct', value: '0.5 / 0.3 / 0' },
        ],
        flowSteps: [
          'Sistem ETH 15m marketini bulur.',
          'sideMode=down oldugu icin yalnizca DOWN tarafinda plan olusturur.',
          'baseUsdc=8 ile her tetik USDC bazli boyutlandirilir.',
          'dcaLevels=3 oldugu icin toplamda daha fazla kademe planlanir.',
          'tp/sl degerleri 0 dan buyuk oldugu icin uygun durumda risk cikislari degerlendirilir.',
        ],
        expectedOutcome:
          'DOWN yonunde daha temkinli birikim yapilir; kapanisa 4 dk kala yeni emir acilmaz.',
      },
      {
        title: 'Ornek 3 - SOL 5m (Up, Shares)',
        summary: 'Hizli markette tek tarafli deneme senaryosu.',
        values: [
          { key: 'asset', value: 'sol' },
          { key: 'timeframe', value: '5m' },
          { key: 'sideMode', value: 'up' },
          { key: 'baseSizing', value: 'shares' },
          { key: 'baseShares', value: '6' },
          { key: 'dcaLevels', value: '2' },
          { key: 'nearStep / stepMult / sizeMult', value: '0.1 / 1.15 / 1' },
          { key: 'minPriceDistanceCent / cutoffMin', value: '1 / 2' },
          { key: 'tpProfitPct / slLossPct / slSpreadPct', value: '0.4 / 0.2 / 0' },
        ],
        flowSteps: [
          'Sistem SOL 5m marketini bulur.',
          'sideMode=up oldugu icin yalnizca UP tarafini izler.',
          'baseShares=6 ve dcaLevels=2 ile hafif bir kademe plani olusur.',
          'nearStep=0.1 ve stepMult=1.15 ile ikinci kademe daha uzak hesaplanir.',
          'tp/sl degerleri pozitif oldugu icin kosul olusursa koruma cikisi devreye girebilir.',
        ],
        expectedOutcome:
          'Hizli periyotta sadece UP tarafinda daha kontrollu test plani calisir.',
      },
    ],
    checklist: [
      'asset, timeframe, sideMode, baseSizing zorunlu.',
      'baseSizing shares ise baseShares; usdc ise baseUsdc > 0 girilmeli.',
      'dcaLevels, nearStep, stepMult, sizeMult, minPriceDistanceCent, cutoffMin bos birakilmaz.',
      'tpProfitPct, slLossPct, slSpreadPct alanlari bos birakilmaz (0 girebilirsin).',
      'Publish oncesi Dogrula butonuyla hatalari kontrol et.',
    ],
  },
};

export const NODE_FIELD_HELP_CONTENT: Partial<Record<string, Record<string, NodeFieldHelpItem>>> = {
  'action.dual_dca': {
    sourceTradeId: {
      title: 'Source Trade ID',
      description: 'Bu stratejinin bagli calisacagi trade kaydinin ID\'si.',
      example: 'Ornek: 1542',
      effect: 'Sistem bu ID\'ye ait trade uzerinden pozisyon ve PnL takibi yapar.',
      tips: ['Bos birakirsan publish/validate hatasi alirsin.'],
    },
    asset: {
      title: 'Coin (asset)',
      description: 'Hangi coin icin market aranacagini belirler.',
      example: 'Ornek: btc',
      effect: 'Sistem bu coin\'in aktif marketini bulur ve slug eslemesi yapar.',
      tips: ['Gecerli degerler: btc, eth, sol, xrp.'],
    },
    timeframe: {
      title: 'Market Period (timeframe)',
      description: 'Market periyodunu secersin. 5m daha sik tetiklenir, 15m daha yavas.',
      example: 'Ornek: 5m',
      effect: 'Hangi periyottaki markette islem yapilacagini belirler.',
      whatHappensIfLowHigh: [
        '5m: Daha sik tetik, hizli karar gerekir.',
        '15m: Daha seyrek tetik, daha filtreli.',
      ],
    },
    sideMode: {
      title: 'Side',
      description: 'Emirlerin hangi yone acilacagini belirler.',
      example: 'Ornek: all',
      effect: 'all: YES ve NO iki tarafa da emir koyar. up: sadece YES. down: sadece NO.',
      simpleExamples: [
        'all → BTC yukselirse YES kazanir, duserse NO kazanir — ikisine de merdiven kurulur.',
        'up → sadece YES tarafinda birikim yapar.',
      ],
    },
    baseSizing: {
      title: 'Base Sizing',
      description: 'Emir boyutunun shares (pay) mi yoksa usdc (dolar) mi olacagini secersin.',
      effect: 'shares secersen baseShares alani kullanilir, usdc secersen baseUsdc kullanilir.',
    },
    baseShares: {
      title: 'Base Shares',
      description: 'Level 0\'daki (ilk) emrin pay miktari. Sonraki levellerde sizeMult ile carpilir.',
      example: 'Ornek: baseShares=10, sizeMult=1.2 → Level 0: 10 pay, Level 1: 12 pay, Level 2: 14.4 pay',
      effect: 'Formul: baseShares x sizeMult^level. Her level\'de emir buyuklugu katlanarak artar.',
      tips: ['Sadece baseSizing=shares iken kullanilir.', '0\'dan buyuk olmali.'],
    },
    baseUsdc: {
      title: 'Base USDC',
      description: 'Level 0\'daki (ilk) emrin USDC tutari. Sonraki levellerde sizeMult ile carpilir.',
      example: 'Ornek: baseUsdc=8, sizeMult=1.2 → Level 0: 8$, Level 1: 9.6$, Level 2: 11.52$',
      effect: 'Formul: baseUsdc x sizeMult^level. Her level\'de dolar bazli emir buyur.',
      tips: ['Sadece baseSizing=usdc iken kullanilir.', '0\'dan buyuk olmali.'],
    },
    basePriceUsdc: {
      title: 'Base Price (USDC)',
      description:
        'Kademelerin hesaplanacagi referans fiyat. Doluysa ilk alim da bu fiyata gelince (cross_below) tetiklenir; bos birakirsan ilk alim aninda yapilir.',
      example: 'Ornek: 0.55 (veya bos birak → market fiyati)',
      effect:
        'Level 0 (ilk alim) basePriceUsdc girildiğinde bu fiyata gelince tetiklenir. Level 1+ fiyatlari bu referans noktasindan nearStep ile asagi hesaplanir.',
      tips: ['0.01-0.99 arasinda olmali.', '55 cent icin 0.55 gir.'],
    },
    dcaLevels: {
      title: 'DCA Levels',
      description:
        'Base (Level 0) haric kac ek kademe olacagini belirler. Base price girildiğinde Level 0 dahil tum seviyeler fiyat dusunce tetiklenir.',
      example: 'Ornek: dcaLevels=3 → Level 0 + Level 1,2,3 = toplam 4 emir',
      effect:
        'Level 1+ her zaman cross_below tetiklidir. Level 0 sadece basePriceUsdc giriliyse cross_below tetiklenir; bossa aninda calisir.',
      tips: ['Gecerli aralik: 1-20.', 'Daha fazla level = daha genis fiyat merdiveni.'],
    },
    nearStep: {
      title: 'Near Step',
      description: 'Level 1\'in referans fiyata olan uzakligi. Sonraki leveller stepMult ile genisler.',
      example: 'Ornek: nearStep=0.05, stepMult=1.0 → Level 1: -0.05, Level 2: -0.10, Level 3: -0.15 (esit aralik). nearStep=0.05, stepMult=1.2 → Level 1: -0.05, Level 2: -0.11, Level 3: -0.18 (artan aralik)',
      effect: 'Formul: stepMult=1 ise nearStep x level (dogrusal). stepMult>1 ise nearStep x (stepMult^n - 1)/(stepMult - 1) (ustel).',
      tips: ['0 ile 1 arasinda olmali (0 ve 1 haric).'],
    },
    stepMult: {
      title: 'Step Mult.',
      description: 'Kademeler arasindaki fiyat mesafesinin buyume carpani. 1 = esit aralik, >1 = her kademe daha uzak.',
      example: 'Ornek: stepMult=1.0 → her level ayni mesafede. stepMult=1.5 → her level bir oncekinden 1.5x daha uzak.',
      effect: '1\'e yakin: Kademeler birbirine yakin, duzgun dagilir. Yuksek: Son kademeler cok uzaklasir.',
      tips: ['1 veya daha buyuk olmali.'],
    },
    sizeMult: {
      title: 'Size Mult.',
      description: 'Her level\'de emir buyuklugunun carpani. 1 = sabit boyut, >1 = her level\'de daha buyuk emir.',
      example: 'Ornek: sizeMult=1 → tum leveller ayni boyut. sizeMult=1.2 → Level 0: 10$, Level 1: 12$, Level 2: 14.4$',
      effect: 'Formul: baseSize x sizeMult^level. Dusuk levellerde kucuk, derin levellerde buyuk emir acilir.',
      tips: ['0\'dan buyuk olmali.', '1 koyarsan tum leveller esit buyuklukte olur.'],
    },
    minPriceDistanceCent: {
      title: 'Min Price Distance (cent)',
      description: 'Emir fiyatinin market fiyatindan ne kadar agresif olacagini belirler (cent cinsinden).',
      example: 'Ornek: 1 cent = 0.01 fark. Market fiyati 0.50 ise → alis emri 0.51\'e koyulur.',
      effect: 'Buy emirlerinde fiyata +distance eklenir, sell emirlerinde -distance cikarilir. Daha agresif fiyat = daha hizli dolum.',
      tips: ['0\'dan buyuk olmali.'],
    },
    cutoffMin: {
      title: 'Cutoff Min',
      description: 'Market kapanisina kac dakika kala yeni emir acmayi durduracagini belirler.',
      example: 'Ornek: cutoffMin=3, market 15:00\'da kapaniyor → 14:57\'den sonra yeni emir acilmaz ve bekleyen kosullu emirler iptal edilir.',
      effect: 'Kapanisa yakin acilan emirler settle riskine girer. Bu deger o riski onler.',
      tips: ['0 veya daha buyuk olmali.', '0 koyarsan son ana kadar emir acilabilir.'],
    },
    tpProfitPct: {
      title: 'TP Profit (USDC)',
      description: 'Toplam pozisyonun USDC cinsinden kar hedefi. Yuzde degil, dolar tutaridir.',
      example: 'Ornek: tpProfitPct=1.5 → toplam unrealized kar 1.50$ veya uzeri olursa tum bekleyen emirler iptal edilir ve pozisyon kapatilir.',
      effect: 'Sistem YES+NO tum acik pozisyonlarin toplam unrealized PnL\'ini hesaplar. PnL >= bu deger ise TP tetiklenir.',
      tips: ['0 veya daha buyuk olmali.', '0 koyarsan TP devre disi kalir.'],
    },
    slLossPct: {
      title: 'SL Loss (USDC)',
      description: 'Toplam pozisyonun USDC cinsinden zarar limiti. Yuzde degil, dolar tutaridir.',
      example: 'Ornek: slLossPct=1.0 → toplam unrealized zarar -1.00$ veya daha kotu olursa tum bekleyen emirler iptal edilir ve pozisyon kapatilir.',
      effect: 'Sistem YES+NO tum acik pozisyonlarin toplam unrealized PnL\'ini hesaplar. PnL <= -bu deger ise SL tetiklenir.',
      tips: ['0 veya daha buyuk olmali.', '0 koyarsan SL devre disi kalir.'],
    },
    slSpreadPct: {
      title: 'SL Spread (USDC)',
      description: 'Ek koruma toleransi icin ayrilan USDC esigi. Su an aktif kullanilmiyor, ileride spread bazli risk icin ayrilmis.',
      example: 'Ornek: 0 (genelde 0 birakilir)',
      effect: 'Risk payload\'inda takip edilir ama su an TP/SL kararini etkilemez.',
      tips: ['0 veya daha buyuk olmali.', 'Kullanmayacaksan 0 gir.'],
    },
    refKey: {
      title: 'Reference Key',
      description: 'Bu node\'un ciktisini flow icinde hangi isimle takip edecegini belirler.',
      example: 'Ornek: dual_dca_btc_5m',
      effect: 'Bos birakirsan node key otomatik kullanilir.',
    },
  },
};
