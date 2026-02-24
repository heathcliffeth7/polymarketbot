# Dextrabot Polymarket Bot - Master Plan

## Amaç
BTC 5m Up/Down marketlerinde, market sonucu beklemeden 5 dakikalık volatiliteyi trade eden bir botu paper-first yaklaşımıyla üretime hazır hale getirmek.

## Kapsam
- Market: BTC 5m Up/Down
- Çalışma modu: Paper + Live-ready
- Stack: Rust
- Veri akışı: CLOB WebSocket + REST fallback
- Depolama: PostgreSQL + Redis
- Deployment: Local server (single instance)

## Kapsam Dışı (V1)
- Multi-asset (ETH vb.)
- Multi-strategy plugin engine
- K8s / distributed worker topology

## Fazlar
1. Faz 0 - Mimari ve operasyon dokümantasyonu (bu klasör)
2. Faz 1 - Paper runtime + doğrulama
3. Faz 2 - Düşük notional controlled live
4. Faz 3 - Parametre optimizasyonu + güvenli ölçekleme

## Başarı Kriterleri
- Deterministik trade state machine
- Reconnect/reconcile hatalarında state bozulmaması
- Risk policy breach durumunda otomatik halt
- Paper sonuçları ile ledger/log tutarlılığı

## Çapraz Referanslar
- Mimarinin bileşenleri: `architecture.md`
- State geçişleri: `state-machine.md`
- Şema: `db-schema.md`
- Risk kuralları: `risk-policy.md`
- Operasyon: `runbook.md`
- Kabul testleri: `acceptance-tests.md`
- Konfigürasyon: `config-spec.md`
- Olay yönetimi: `incident-playbooks.md`
- Terimler: `glossary.md`
