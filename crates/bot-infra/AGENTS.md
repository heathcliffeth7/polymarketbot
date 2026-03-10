# bot-infra AGENTS

## Kapsam
- `crates/bot-infra/**`

## Kurallar
- Postgres, exchange HTTP/WS, config load/validate, signer, claim ve market-data entegrasyonları burada yaşar.
- Repository ve SQL değişiklikleri burada kalmalı; runner trait çağırmalı, sorgu yazmamalı.
- WS/fill işleme idempotency kurallarını koru.
- Hassas config yalnızca `enc:v1:` veya env-backed biçimde kalmalı; doğrulama fail-fast olmalı.
- DB kontratı değişirse migration ve frontend query tüketicilerini aynı değişiklikte güncelle.
