# Trade Flow Query AGENTS

## Kapsam
- `frontend/src/lib/queries/trade-flow/**`

## Kurallar
- `validation-actions.ts` tek switchboard dosyasi gibi buyumemeli; validatorlari node ailesine gore ayir.
- Varsayilan ayirim eksenleri: `trigger.market_price`, `trigger.open_positions`, `trigger.position_drawdown`, `action.resolve_market`, `action.place_order`, `action.dual_dca`.
- Ortak parse/normalization yardimcilari `shared.ts` veya uygun yeni yardimci modullerde kalmali; node-spesifik kurallari ortak dosyaya geri yigma.
- Yeni veya dokunulan query/validation dosyalari 1000 satiri asmamali; 900+ satirda once extraction, sonra yeni kural ekleme uygula.
- Backend trade-flow kontrati degisirse ilgili validator, tip ve API tuketicilerini ayni degisiklikte guncelle.
