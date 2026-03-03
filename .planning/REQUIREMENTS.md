# Requirements: Dextrabot

**Defined:** 2026-03-03
**Core Value:** Deterministic trade state machine with correct price-driven signals and risk guardrails

## v1.1 Requirements

Requirements for tick bug fix milestone. Each maps to roadmap phases.

### Trigger Evaluation

- [ ] **TRIG-01**: Confirmation timer sıfırlanmalı — fiyat trigger zone'dan çıktığında pending confirmation state (cross_pending_at) temizlenmeli, timer out-of-zone süresince koşmamalı
- [ ] **TRIG-02**: first_tick_threshold + confirmation gate etkileşimi güvenli olmalı — auto_scope+once modunda market açılışında fiyat zaten threshold üstündeyse, confirmation gate bunu güvenilir şekilde filtrelemeli (şu an fragile interaction)
- [ ] **TRIG-03**: Confirmation gate testleri yazılmalı — out-of-zone reset, re-entry timing, first_tick+confirmation etkileşimi için unit testler

### Price Data Integrity

- [ ] **PRCE-01**: previous_price legacy fallback kaldırılmalı — per-token `previous_price_{token_id}` key'i bulunamazsa bare `previous_price` key'ine düşmemeli, farklı token'dan stale fiyat kullanımını engellemeli
- [ ] **PRCE-02**: Reconcile edge case — WS tick ve REST snapshot aynı timestamp'e sahipse davranış belirli olmalı (şu an `>=` ile WS tercih ediliyor, eşitlik durumu test edilmeli)

## v2 Requirements

Şu an kapsam dışı, gelecek milestone için not.

- **TICK-01**: Real WS MarketDataProvider implementasyonu (şu an sadece MockMarketDataProvider var)
- **TICK-02**: Stale data threshold konfigürasyonu (reconcile'da max stale süresi)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Yeni strategy ekleme | Bu milestone sadece bug fix |
| State machine değişikliği | Mevcut 11-state modeli doğru çalışıyor |
| Frontend tick gösterimi | Backend-only milestone |
| WS reconnect logic | ws.rs'deki reconnect ayrı bir konu |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| TRIG-01 | Phase 6 | Pending |
| TRIG-02 | Phase 6 | Pending |
| TRIG-03 | Phase 6 | Pending |
| PRCE-01 | Phase 7 | Pending |
| PRCE-02 | Phase 7 | Pending |

**Coverage:**
- v1.1 requirements: 5 total
- Mapped to phases: 5
- Unmapped: 0

---
*Requirements defined: 2026-03-03*
*Last updated: 2026-03-03 after roadmap creation*
