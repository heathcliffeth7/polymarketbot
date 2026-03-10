# Scripts AGENTS

## Kapsam
- `scripts/*.sh`, `scripts/*.py`

## Kurallar
- Shell scriptlerde `#!/usr/bin/env bash` ve `set -euo pipefail` standardını koru.
- Var olan helper scriptleri genişletmeyi, aynı işi yapan yeni script eklemeye tercih et.
- Setup scriptleri idempotent kalmalı; tekrar çalıştırıldığında güvenli davranmalı.
- Gerçek servis adları `dextrabot` ve `dextrabot-frontend`; eski veya uydurma isim kullanma.
- Repo içindeki scriptlere sudo parolası gömme; parola sadece agent oturumunda komut çalıştırırken kullanılabilir.

## Doğrulama
- Düzenlenen shell scriptlerde en az `bash -n <script>` çalıştır.
- Setup veya health scripti değiştiyse ilgili hedef komutu kuru çalıştırma ya da güvenli doğrulama ile kontrol et.
