export type DcaMarketSelectionMode =
  | 'manual_slug'
  | 'manual_slug_list'
  | 'auto_group_top_n'
  | 'auto_scope';

export type DcaSideMode =
  | 'one_sided'
  | 'two_sided_pair'
  | 'multi_outcome_basket';

export interface DcaResolvedOutcome {
  label: string;
  normalizedLabel: string;
  tokenId: string;
  bestBid: number | null;
  bestAsk: number | null;
  mid: number | null;
  liquidity: number | null;
}

export interface DcaResolvedMarket {
  slug: string;
  title: string;
  status: string;
  isClosed: boolean;
  isResolved: boolean;
  isBinary: boolean;
  endTime: string | null;
  volume: number | null;
  liquidity: number | null;
  outcomes: DcaResolvedOutcome[];
  pairEligible: boolean;
  pairEligibilityReason: string;
}

export interface DcaSelectedOutcome {
  slug: string;
  outcomeLabel: string;
  tokenId: string;
}

export interface DcaResolveMarketRequest {
  input: string;
}

export interface DcaResolveMarketsRequest {
  inputs: string[];
}

export interface DcaPreviewRequest {
  market?: DcaResolvedMarket | null;
  selectedOutcomes?: DcaSelectedOutcome[];
  dcaConfig?: Record<string, unknown>;
}

export interface DcaLadderPreviewLevel {
  level: number;
  outcomeLabel: string;
  tokenId: string;
  priceCent: number;
  shares: number;
  estimatedCostUsdc: number;
}
