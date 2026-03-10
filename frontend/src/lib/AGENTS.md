# frontend/lib AGENTS

## Kapsam
- `frontend/src/lib/**`

## Kurallar
- Auth, config, db, query orkestrasyonu, systemctl ve ortak tip yardımcıları burada yaşar.
- Side effect içeren mantığı burada tut; component katmanına sızdırma.
- `systemctl.ts` frontend servis kontrolü için tek soyutlama olarak kalmalı; `dextrabot` defaultu ve `sudo -n` semantiği korunmalı.
- `config.ts` ve auth yardımcıları `CONFIG_ENCRYPTION_KEY`, `AUTH_SECRET` ve `/etc/dextrabot` env akışıyla uyumlu kalmalı.
- 900+ satırlık büyük yardımcı dosyalara yeni dal eklemeden önce modüle böl.

## Doğrulama
- Bu alandaki değişikliklerde `cd frontend && npm run lint && npm run build`
