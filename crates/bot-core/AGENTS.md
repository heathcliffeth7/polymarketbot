# bot-core AGENTS

## Kapsam
- `crates/bot-core/**`

## Kurallar
- Bu crate saf domain katmanıdır: tipler, state machine, risk ve strateji.
- DB, HTTP, filesystem, shell, systemd veya exchange client kodu ekleme.
- Deterministik ve test edilebilir fonksiyonları tercih et.
- Geçiş kuralları state machine yardımcıları üzerinden kalmalı; yan yoldan state değiştirme.
- Domain hataları için `thiserror`, üst katman sarmalaması için `anyhow` yaklaşımını koru.
