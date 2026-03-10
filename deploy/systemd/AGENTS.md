# systemd AGENTS

## Kapsam
- `deploy/systemd/*`

## Kurallar
- Backend servis adı `dextrabot`; binary yolu `/home/heathcliff/polymarketbot/target/release/bot-runner`; env dosyası `/etc/dextrabot/dextrabot.env`.
- Frontend servis adı `dextrabot-frontend`; workdir `/home/heathcliff/polymarketbot/frontend`; env dosyası `/etc/dextrabot/dextrabot-frontend.env`.
- `setup_server.sh`, `setup_frontend_service.sh`, `README.md` ve unit dosyaları birbiriyle uyumlu kalmalı.
- Hardening ayarlarını sebepsiz gevşetme; frontend unitindeki `NoNewPrivileges=false` seçimi `sudo -n systemctl` için bilinçli.
- `polymarket-frontend` gibi eski servis isimlerini geri getirme.

## Doğrulama
- Unit dosyası değiştiyse `printf '2100100\n' | sudo -S systemctl daemon-reload` çalıştır.

## Restart
- Backend unit değiştiyse `dextrabot` restart et.
- Frontend unit değiştiyse `dextrabot-frontend` restart et.
