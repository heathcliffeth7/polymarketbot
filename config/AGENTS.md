# Config AGENTS

## Kapsam
- `config/*.toml`, `config/*.example` ve aynı klasördeki çalışma yedekleri.

## Kurallar
- Gizli değerleri düz metin yazma; `enc:v1:` veya tam `*_env` eşlemesini kullan.
- `exchange.toml` için doğrudan şifreli credential seti tercih edilir; env fallback kullanılacaksa `api_address_env`, `api_key_env`, `api_secret_env`, `api_passphrase_env` birlikte bulunmalı.
- `.example` dosyalarını gerçek şema ile uyumlu tut; `.bak.*` dosyalarına kullanıcı istemeden dokunma.
- Config shape değişirse ilgili `bot-infra` loader/validator kodunu ve gerekiyorsa frontend settings yüzünü aynı değişiklikte güncelle.
- `BOT_CONFIG_DIR` ve `/etc/dextrabot/*.env` akışıyla çelişen yeni yol/anahtar uydurma.

## Doğrulama
- Runtime config etkilediğinde `cargo check` çalıştır.
- Auth, encryption veya frontend tarafından okunan env/config etkilenirse `cd frontend && npm run lint && npm run build` çalıştır.

## Restart
- Bot runtime config değiştiyse `dextrabot` restart et.
- Frontend auth/config akışı değiştiyse `dextrabot-frontend` restart et.
