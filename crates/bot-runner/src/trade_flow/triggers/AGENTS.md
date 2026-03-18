# Trade Flow Trigger AGENTS

## Kapsam
- `crates/bot-runner/src/trade_flow/triggers/**`

## Kurallar
- Trigger dosyalari karar agaci gibi buyumemeli; yeni davranisi mevcut dev fonksiyonun icine eklemek yerine isimli yardimci modullere cikar.
- `market_price.rs` degisirse mantigi su sinirlarda ayir: `auto_scope` secimi, WS step parse/guard, cross-confirmation evaluation, `once/repeat` state, protection evaluation ve event recording.
- `market_price.rs` veya kardes trigger dosyalarini 1400+ satira iterken yeni extraction yap; 1500 satiri gecen halde birakma.
- Yeni numbered `part_###` dosyasi acma; `mod helpers;`, `mod evaluation;` gibi anlamli adlar kullan.
- Trigger runtime semantigini degistiren her degisiklikte mevcut `src/tests/` kapsamindan ilgili senaryoyu genislet veya yeni hedefli test ekle.
