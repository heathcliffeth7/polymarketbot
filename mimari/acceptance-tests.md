# Acceptance Tests

## Fonksiyonel
1. Yeni market algılama doğru
2. Entry price hit -> order place
3. Partial fill -> state doğru
4. TP fill -> trade settled
5. Aggressive SL -> renewal akışı

## Dayanıklılık
1. WS disconnect -> REST fallback
2. Duplicate fill -> idempotent işlem
3. API transient error -> retry/backoff
4. Restart -> reconcile + resume
5. Mock exchange harness üzerinde deterministic replay

## Risk
1. Daily loss breach -> halt
2. Consecutive loss breach -> halt
3. Stale data -> block
4. Kill-switch -> instant block

## Başarı Eşiği (Paper -> Live)
- Minimum trade sample: 100
- Critical state mismatch: 0
- Reconcile başarısızlık oranı: < %1
- Max drawdown: policy limit içinde
- Order reject (non-policy): kabul edilebilir eşiğin altında
- Go/No-Go gate script başarılı (`scripts/go_no_go.sh`)
