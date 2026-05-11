# 14 - Claim Sweep ve Funds Activation

Güncelleme tarihi: 2026-05-01

## Amaç

Claim sweep, resolved marketlerdeki redeemable pozisyonları otomatik nakde çevirmek için vardır. Güncel akışta buna funds activation eklendi: Builder/Relayer akışında Safe üzerinde bekleyen USDC.e bakiyesi pUSD'ye activate edilir.

Bu bölüm trading stratejisinden ayrı operasyon lifecycle'ıdır.

## Polymarket Yüzeyi

- CTF redeem: winning conditional token'lar `redeemPositions` ile collateral'a döner.
- Builders Relayer: Safe/proxy wallet üzerinden gasless onchain işlem gönderir.
- Market-maker setup: Polygon USDC.e, Safe wallet ve gerekli approval/onramp adresleri operasyonel ön koşuldur.

Bu yüzeyler CLOB order execution değildir. Claim/redeem işlemi token lifecycle operasyonudur.

## Execution Mode

| `execution_mode` | Anlamı |
|---|---|
| `direct` | Backend private key ile doğrudan onchain redeem gönderir |
| `builder_relayer` | Frontend internal adapter üzerinden Polymarket Builder Relayer kullanır |
| `relayer_api_key` | Relayer API key imzalı Safe transaction gönderir |

Funds activation sadece `builder_relayer` veya `relayer_api_key` için anlamlıdır. `direct` modda Safe relayer activation beklenmez.

## Config Alanları

```toml
enabled = true
execution_mode = "relayer_api_key"
collateral_token_address = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"
auto_activate_funds = true
activate_min_usdc = 0.01
usdce_token_address = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"
pusd_token_address = "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB"
collateral_onramp_address = "0x93070a847efEf7F70739046A929D47a521F5B8ee"
min_claim_usdc = 0.0
```

Önemli değişiklik:

- `collateral_token_address` artık USDC.e adresine ayarlı olmalıdır.
- pUSD ayrı `pusd_token_address` olarak tutulur.
- `activate_min_usdc` Safe üzerinde bekleyen USDC.e küçükse aktivasyonu atlamak için kullanılır.
- `min_claim_usdc = 0.0` dust claim'i de kuyruğa alabilir.

## Sweep Akışı

```text
dashboard claim sweep
  -> redeemable positions keşfedilir
  -> auto_claim_jobs pending/retry olarak queue edilir
  -> AutoClaimService redeem submit eder
  -> receipt_confirmed event'i gelir
  -> auto_activate_funds açıksa funds activation denenir
```

Queue status:

- `pending`
- `retry`
- `processing`
- `submitted`
- `failed`
- `claimed`

Dashboard bu sayıları ve son hatayı gösterir.

## Funds Activation Akışı

Manual UI:

```text
Dashboard -> Claim Sweep Card -> Activate Funds
  -> POST /api/claim/activate-funds
  -> Safe USDC.e / pUSD balance okunur
  -> allowance yetersizse approve transaction eklenir
  -> collateral onramp wrap transaction eklenir
  -> relayer ile Safe transaction gönderilir
```

Internal adapter:

```text
AutoClaimService
  -> POST /api/internal/claim/activate-funds
  -> Bearer CLAIM_RELAYER_ADAPTER_TOKEN
  -> userId + ownerAddress
```

Adapter URL env:

- `CLAIM_RELAYER_ADAPTER_URL`
- `CLAIM_FUNDS_ACTIVATION_ADAPTER_URL`
- `CLAIM_RELAYER_ADAPTER_TOKEN`

## Eventler

| Event | Anlamı |
|---|---|
| `receipt_confirmed` | Redeem tx confirmed |
| `funds_activated` | USDC.e -> pUSD activation submitted |
| `funds_activation_skipped` | Balance threshold altında veya aktivasyon gerekmedi |
| `funds_activation_failed` | Relayer, Safe, config veya onchain hata |

Payload alanları:

- `owner_address`
- `condition_id`
- `activated_amount_usdc`
- `approve_tx_hash`
- `wrap_tx_hash`
- `usdce_balance`
- `pusd_balance`
- `message`

## Sık Hatalar

| Hata | Yorum |
|---|---|
| `relayer_wallet_activation_required` | Relayer işlemden önce funds activation istiyor |
| HTML hata sayfası | Relayer adapter URL/auth yanlış veya upstream HTML döndü |
| `owner_address_mismatch` | Request owner Safe adresiyle eşleşmiyor |
| `configured_safe_mismatch` | Private key'den türeyen Safe config ile uyuşmuyor |
| `funds_activation_mode_unsupported` | `direct` modda activation deneniyor |
| `unsupported_chain` | Safe multisend adresi desteklenmeyen chain |

UI hata formatı uzun raw relayer mesajını operatör cümlesine çevirir. Raw error gerekiyorsa `auto_claim_events.payload_json` okunmalıdır.

## Operatör Checklist

1. `execution_mode` doğru mu?
2. `claim.user_address`, private key ve `exchange.gnosis_safe_address` aynı Safe zincirini veriyor mu?
3. Safe üzerinde USDC.e balance `activate_min_usdc` üstünde mi?
4. `CLAIM_RELAYER_ADAPTER_TOKEN` backend ve frontend internal route için aynı mı?
5. Son event `funds_activation_failed` ise dashboard error ile DB payload aynı mı?
6. Claim cash PnL bekleniyorsa redeem confirmed ve funds activation ayrı raporlanmalı.

## Kaynak Notu

Kod referansları:

- `crates/bot-infra/src/claim.rs`
- `crates/bot-infra/src/claim_relayer.rs`
- `frontend/src/lib/claim-relayer.ts`
- `frontend/src/lib/claim-funds-activation.ts`
- `frontend/src/components/dashboard/claim-sweep-card.tsx`
- `frontend/src/app/api/claim/activate-funds/route.ts`
- `frontend/src/app/api/internal/claim/activate-funds/route.ts`

Polymarket docs path'leri:

- `/developers/CTF/overview`
- `/developers/CTF/redeem`
- `/developers/builders/relayer-client`
- `/developers/market-makers/setup`
