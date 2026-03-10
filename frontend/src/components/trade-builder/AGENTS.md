# Trade Builder Components AGENTS

## Kapsam
- `frontend/src/components/trade-builder/**`

## Kurallar
- Bu alan zaten büyük; varsayılan hareket yeni mantığı kardeş modüllere bölmek olmalı.
- Canvas render, inspector formu, flow engine eylemleri ve import/export mantığını ayrı tut.
- `editor-body.tsx` ve benzeri buyuk componentler render/wiring katmani olarak kalmali; graph aksiyonlari, form state, keyboard davranisi ve secim yardimcilari ayri modullerde yasamali.
- Bir component 900+ satira geliyorsa yeni JSX veya davranisi ayni dosyaya yigmak yerine alt component, hook veya helper cikar.
- Yeniden kullanılabilir graph veya node yardımcılarını component body içine gömme; dosya dışına ya da `src/lib/trade-flow-config-mappers` alanına taşı.
- Builder config semantiği backend trade-flow şemasıyla uyumlu kalmalı.
- Config shape değişirse mapper, query ve API tüketicilerini aynı değişiklikte güncelle.
