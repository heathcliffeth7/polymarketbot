# Trade Builder AGENTS

## Kapsam
- `crates/bot-runner/src/trade_builder/**`

## Kurallar
- Exit lifecycle mantigini tek dosyada buyutme; math, fill resolution, retry-state ve sibling priority ayrimini koru.
- Public davranisi degistirmeyen refactorlarda isimleri ve event semantiklerini koru.
- Yeni numbered `part_###` dosyasi acma; sorumluluga gore isimli moduller kullan.
- Trade-builder runtime semantigini etkileyen her degisiklikte ilgili `src/tests/place_order_*` kapsamindan hedefli test ekle veya mevcut testi guncelle.
