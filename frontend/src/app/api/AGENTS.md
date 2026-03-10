# API AGENTS

## Kapsam
- `frontend/src/app/api/**`

## Kurallar
- Route handlerlar `NextRequest` ve `NextResponse` kullanır.
- Login/auth bootstrap dışındaki endpointlerde auth kontrolünü koru.
- DB erişimi `@/lib/db`, config okuma/yazma `@/lib/config`, servis kontrolü `@/lib/systemctl` üzerinden gitmeli.
- Handler içinde dağınık `child_process` veya shell komutu yazma; ortak wrapper kullan.
- Hata yanıtları yapılandırılmış JSON dönmeli; service-control akışında reason ve reasonCode alanlarını koru.
