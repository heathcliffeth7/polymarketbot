# Incident Playbooks

## 1) WebSocket Disconnect
1. Disconnect algıla
2. Reconnect dene (bounded backoff)
3. REST snapshot çek
4. Open order ve trade state reconcile et
5. Stale ise yeni entry block

## 2) Signature / Nonce Hataları
1. Saat senkronu kontrolü
2. Nonce durumu kontrolü
3. Credential/funder/signature type doğrulaması
4. Test order yerine dry validation

## 3) DB Erişim Sorunu
1. Write path'leri fail-closed moda al
2. Yeni trade açmayı durdur
3. DB geri gelince reconcile + backlog flush

## 4) Order State Mismatch
1. Exchange order status çek
2. Local state ile diff çıkar
3. Deterministic correction uygula
4. Audit log ve risk event yaz

## 5) Kill-switch Olayı
1. Yeni entry kapat
2. Açık orderları güvenli moda al
3. Operatör onayı olmadan resume etme
