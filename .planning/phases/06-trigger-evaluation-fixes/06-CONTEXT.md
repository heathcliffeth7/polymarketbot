# Phase 6: Trigger Evaluation Fixes - Context

**Gathered:** 2026-03-03
**Status:** Ready for planning

<domain>
## Phase Boundary

Confirmation gate timer ve first_tick_threshold davranış hatalarını düzelt. Doğru davranışı tanımla, uygula ve test et. Yeni trigger tipi veya capability eklenmeyecek — mevcut `trigger.market_price` node'unun doğru çalışması sağlanacak.

Requirements: TRIG-01, TRIG-02, TRIG-03

</domain>

<decisions>
## Implementation Decisions

### Timer Reset Davranışı (TRIG-01)
- Fiyat trigger zone'dan çıktığında `cross_pending_at` ve ilgili pending state tamamen temizlenmeli
- Timer out-of-zone süresince koşmaya devam ETMEMELİ (mevcut bug — satır 4306)
- Reset sonrası zone'a geri girişte yeni cross algılanmalı, timer sıfırdan başlamalı
- Reset sayısı için limit yok — her zone çıkışında sıfırla, basit ve tahmin edilebilir
- Reset olduğunda `info` seviyesinde `CROSS_PENDING_RESET` logu yazılmalı

### first_tick + Confirmation Etkileşimi (TRIG-02)
- auto_scope+once modunda first_tick'e izin vermeye devam et (mevcut yaklaşım korunsun)
- Confirmation gate güçlendirilmeli: first_tick'ten gelen trigger da confirmation_secs süresince zone'da kalma kontrolüne tabi
- Confirmation gate'in first_tick'i güvenilir şekilde filtrelediğinden emin olunmalı
- first_tick ile gerçek cross arasında farklı confirmation süresi uygulanıp uygulanmayacağı Claude'un takdirine bırakıldı

### Claude's Discretion
- Log seviyesi detayları (warn vs info ayrımı)
- first_tick için farklı confirmation süresi stratejisi (aynı / çift süre)
- Confirmation gate'in internal state yönetimi detayları
- Test helper/fixture tasarımı

</decisions>

<specifics>
## Specific Ideas

- Satır 4306'daki yorum ("Out-of-zone tick'lerde pending state korunur") doğrudan bug'ın konumu
- `still_in_zone` kontrolü (4223-4227) doğru çalışıyor ama zone çıkışında pending state temizlenmeli
- `evaluate_trigger_market_price_condition()` fonksiyonu stateless — doğru. Sorun confirmation gate state yönetiminde

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `evaluate_trigger_market_price_condition()` (main.rs:234-270): Stateless cross checker — bu fonksiyon doğru çalışıyor, değişiklik gerektirmez
- `set_flow_node_state()` / `remove_flow_node_state()`: State manipulation helper'ları zaten var, reset için kullanılabilir
- `flow_node_state_string()`: Pending state okuma için mevcut helper
- Mevcut testler (main.rs:8341-8378): Temel cross/first_tick testleri — genişletilecek

### Established Patterns
- Flow node state JSON context üzerinden yönetiliyor (`run_spec.context`)
- Per-token state key'leri: `cross_pending_at_{token_id}`, `cross_pending_price_{token_id}`
- Structured logging: tracing macros ile field-based log (run_id, node_key, price, market)

### Integration Points
- Confirmation gate: main.rs:4215-4308 — bu bloğun else branch'inde (4305-4308) zone çıkış tespiti yapılacak
- `should_enqueue` flag: Gate sonucu bu flag üzerinden flow'a iletiliyor
- `final_eval_mode`: "cross_confirmed" string'i downstream'de kullanılıyor — yeni mode eklenmesi gerekebilir ("cross_reset" vb.)

</code_context>

<deferred>
## Deferred Ideas

- Real WS MarketDataProvider implementasyonu — v2 (TICK-01)
- Stale data threshold konfigürasyonu — v2 (TICK-02)
- previous_price legacy fallback kaldırma — Phase 7 (PRCE-01)

</deferred>

---

*Phase: 06-trigger-evaluation-fixes*
*Context gathered: 2026-03-03*
