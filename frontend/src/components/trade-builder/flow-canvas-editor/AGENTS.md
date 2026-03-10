# Flow Canvas Editor AGENTS

## Kapsam
- `frontend/src/components/trade-builder/flow-canvas-editor/**`

## Kurallar
- `editor-body.tsx` render/wiring sahibi olarak kalmali; yeni graph mutasyonlari `actions.ts`, form state davranislari `form-state.ts`, paylasilan hesaplamalar `helpers.ts` veya yeni anlamli modullere tasinmali.
- Market/outcome secimi, keyboard akislari ve layout hesaplari component body icine inline buyutulmemeli.
- Yeni veya dokunulan editor dosyalari 1000 satiri asmamali; 900+ satirda once extraction yap.
- Yeni alt dosya gerekiyorsa `part_###` yerine davranisi anlatan isimler kullan.
