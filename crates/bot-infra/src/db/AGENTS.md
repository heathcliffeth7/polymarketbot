# DB Layer AGENTS

## Kapsam
- `crates/bot-infra/src/db/**`

## Kurallar
- Repository API'lerini tek buyuk dosyada biriktirme; sorgulari sorumluluga gore ayir.
- `trade_flow.rs` degisirse mantigi ayri modullere bol: `definitions`, `runs`, `steps`, `events`, `dual_dca_jobs`, `dual_dca_legs`.
- Definition CRUD, run status/context, queue/claim mantigi ve DCA persistence ayni dosyada buyumemeli.
- Yeni veya dokunulan DB modulleri 1000 satiri asmamali; 900+ satira gelen dosyaya yeni sorgu eklemeden once extraction yap.
- Runner tarafinin kullandigi public repository imzalarini korurken ic organizasyonu alt modullere tasimayi tercih et.
