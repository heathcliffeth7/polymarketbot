# Hooks AGENTS

## Kapsam
- `frontend/src/hooks/**`

## Kurallar
- Hooklar veri alma, polling ve UI için kullanılabilir durum birleştirme katmanıdır.
- SWR ve mevcut `use-polling` davranışını kullan; component içinde ad-hoc interval kurma.
- Hook dönüşleri typed data ile loading/error bilgisini birlikte vermeli.
- Aynı endpoint çağrısını kopyalamak yerine hookları compose et.
- API payload değişirse kullanan componentleri aynı patchte güncelle.
