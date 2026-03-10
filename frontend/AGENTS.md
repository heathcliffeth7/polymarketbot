# Frontend AGENTS

## Kapsam
- `frontend/**`

## Kurallar
- Bu alan Next.js 16 App Router + React 19 kullanır; mevcut klasör ayrımını koru.
- UI dosyaları doğrudan DB, shell veya systemd çağrısı yapmaz; bu işler `src/lib` ve API route katmanında kalır.
- `src/components/trade-builder/flow-canvas-editor/editor-body.tsx`, `src/components/trade-builder/flow-engine-panel.tsx` ve `src/lib/config.ts` zaten büyük; yeni karmaşık mantık eklemeden önce kardeş modüle böl.
- Alias olarak `@/` kullan; gereksiz relatif import zinciri büyütme.

## Doğrulama
- `cd frontend && npm run lint && npm run build`

## Restart
- Production davranışını etkileyen frontend değişikliklerinde `dextrabot-frontend` restart et.
