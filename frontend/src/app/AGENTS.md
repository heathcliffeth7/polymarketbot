# app AGENTS

## Kapsam
- `frontend/src/app/**`

## Kurallar
- App Router sayfa/layout dosyalarını ince tut; ağır iş mantığını hook, component veya `lib` içine taşı.
- `login` dışındaki sayfalar korumalı akış varsayar; auth davranışı middleware ve session kontrolleriyle uyumlu kalmalı.
- Sayfa dosyasında doğrudan shell, DB havuzu veya systemctl çağrısı yazma.
- Route shape değişirse ilgili hook, component ve tipleri aynı patchte güncelle.
