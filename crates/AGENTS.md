# Crates AGENTS

## Kapsam
- `crates/**`

## Kurallar
- Katman yönü korunur: `bot-core` saf domain, `bot-infra` I/O ve entegrasyon, `bot-runner` orkestrasyon, `mock-exchange` test fixture.
- `bot-runner` içinde raw SQL yazma; DB erişimi `bot-infra/src/db/` altında kalmalı.
- Ortak tipleri yukarı değil mümkün olan en alt bağımlılık katmanına taşı.

## Doğrulama
- Her Rust değişikliğinde en az `cargo check` çalıştır.
- Uygunsa `cargo test -p bot-core -p bot-infra -p mock-exchange` da çalıştır.

## Restart
- Runtime davranışını etkileyen crate değişikliklerinde release build sonrası `dextrabot` restart et.
