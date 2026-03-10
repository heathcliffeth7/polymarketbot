# bot-runner AGENTS

## Kapsam
- `crates/bot-runner/**`

## Kurallar
- Bu crate orkestrasyon, trigger döngüleri, trade-flow runtime ve process entrypoint sahibidir.
- Domain kurallarını yeniden yazma; `bot-core` ve `bot-infra` soyutlamalarını kullan.
- Raw SQL veya config parse bypass ekleme.
- Büyük runtime mantığını `trade_flow/`, `tests/` ve `lib_parts/` gibi alt modüllerde tut; tek dosyada yığma.
- Market price trigger veya confirmation gate değişirse mevcut `src/tests/` kapsamını gözden geçir.

## Doğrulama
- `cargo check`
- Mümkünse hedefli testler, ardından `cargo build --release -p bot-runner`

## Restart
- Build sonrası `dextrabot` restart et.
