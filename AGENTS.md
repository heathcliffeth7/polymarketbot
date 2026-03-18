# polymarketbot AGENTS

## Kapsam
- Bu dosya repo geneli için varsayılan kuralları verir. Daha derindeki `AGENTS.md` dosyaları daha spesifik davranır.

## Genel
- Agent yanıtlarını Türkçe yaz; komut, env, dosya adı ve kod kimliklerini aynen bırak.
- Codex yeni veya dokunulan el yazımı kaynak dosyaları 1500 satırı aşacak halde bırakmamalı.
- Bir değişiklik dosyayı 1400+ satıra taşıyorsa yeni mantığı aynı dosyaya yığmadan önce sorumluluğa göre modüle böl.
- Sırf satır sayısını düşürmek için anlamsız `part_###` veya benzeri geçici dosya bölmeleri açma; isimli modül/dizin yapısı kur.
- Bu sınır `target/`, `frontend/.next/`, `node_modules/`, lock dosyaları ve diğer generated/vendor çıktılar için geçerli değil.
- Yeni talimat gerekiyorsa mevcut AGENTS ağacına ekle; tek dosyada yığılma yapma.
- `AGENTS.override.md` yerine normal `AGENTS.md` zinciriyle daha dar kapsamlı kurallar eklemeyi tercih et.
- Kullanıcının mevcut değişikliklerini geri alma; kirli worktree bekle.
- Terminalde sudo gerekirse `printf '2100100\n' | sudo -S <komut>` kullan; bu parolayı scriptlere, uygulama koduna veya env dosyalarına gömme.

## Doğrulama
- Rust/backend değişikliklerinde en az `cargo check`, ardından `cargo build --release -p bot-runner` çalıştır.
- Frontend değişikliklerinde `cd frontend && npm run lint && npm run build` çalıştır.
- Migration veya config şeması değiştiyse ilgili scriptleri kullan ve sonucu raporla.

## Restart
- `crates/`, `config/`, `migrations/`, `scripts/setup_server.sh` veya `deploy/systemd/dextrabot.service` etkilenirse build sonrası `dextrabot` servisini restart et ve `systemctl is-active` ile doğrula.
- `frontend/`, `deploy/systemd/dextrabot-frontend.service` veya frontend env/setup akışı etkilenirse `dextrabot-frontend` servisini restart et ve `systemctl is-active` ile doğrula.
- Hem backend hem frontend etkileniyorsa iki restart akışını da çalıştır.
