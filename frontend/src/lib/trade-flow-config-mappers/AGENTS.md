# Trade Flow Mapper AGENTS

## Kapsam
- `frontend/src/lib/trade-flow-config-mappers/**`

## Kurallar
- Bu alan builder UI durumu ile kalıcı trade-flow config arasında pure mapping ve validation glue katmanıdır.
- DB, network, React component veya shell/systemctl mantığı ekleme.
- `schemas.ts`, `node-config.ts`, `edge-config.ts`, `drafts.ts`, `utils.ts` ayrımını koru.
- Yeni node tipi eklerken önce küçük helper/modül çıkar; `node-config.ts` dosyasını daha da şişirme.
- Mapper değişikliği varsa builder UI ve backend tüketici beklentileriyle birlikte güncelle.
