# frontend/src AGENTS

## Kapsam
- `frontend/src/**`

## Kurallar
- `app/` rota ve sayfa, `components/` UI, `hooks/` veri akışı, `lib/` paylaşılan yardımcılar için kalmalı.
- Component içinde raw SQL, raw fetch veya shell/systemctl mantığı yazma.
- Ortak tip, mapper ve query builder varken kopya mantık üretme.
- Sunucu ve istemci ayrımını net tut; side effect içeren mantığı `lib/` tarafına taşı.
- Yeni dosyalar ve büyüyen modüller 1000 satır sınırını aşmamalı.
