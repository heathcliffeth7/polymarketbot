# Migrations AGENTS

## Kapsam
- `migrations/*.sql`

## Kurallar
- Migrationlar append-only ilerler; mevcut dosyaları geçmiş davranışı bozacak şekilde yeniden yazma.
- Yeni prefix `count + 1` ile değil mevcut en büyük numaraya göre seçilir; repo bugün `026` seviyesinde olduğu için sonraki yeni migration `027_*` olmalı.
- Mevcut çift `021` dosyasını yeniden kullanma veya yeniden numaralama girişiminde bulunma.
- Şema tasarımında `TIMESTAMPTZ`, `TEXT`, `JSONB` ve gerekli index/unique kısıtlarını tercih et.
- DROP/rename gerekiyorsa güvenli backfill ve uyumluluk planı olmadan yapma.

## Doğrulama
- `DATABASE_URL` ile `./scripts/apply_migrations.sh` veya hedef `psql -f` akışını çalıştır.
- Şemaya bağlı Rust/frontend kodu aynı değişiklikte güncellenmiş olmalı.

## Restart
- Migration uygulandıysa doğrulama sonrası `dextrabot` restart et.
