# Bot Feature Guide

This directory is the feature index for Dextrabot's trade-flow runtime, guards, pair-lock strategies, DCA flows, analytics, Telegram notifications, and claim operations.

## Recommended Reading Order

1. [node-ozellikleri.md](./node-ozellikleri.md) - node-level overview.
2. [yeni-ozellikler.md](./yeni-ozellikler.md) - recent feature summary.
3. [senaryolar/01-market-dongusu-ve-auto-scope.md](./senaryolar/01-market-dongusu-ve-auto-scope.md) - market cycle and auto-scope behavior.
4. [senaryolar/02-giris-trigger-ve-zamanlama.md](./senaryolar/02-giris-trigger-ve-zamanlama.md) - triggers, timing profiles, and firing modes.
5. [senaryolar/03-emir-gonderimi-sizing-ve-fill.md](./senaryolar/03-emir-gonderimi-sizing-ve-fill.md) - order sizing, submission, and fill handling.
6. [senaryolar/04-ptb-guard-ve-iv-mismatch.md](./senaryolar/04-ptb-guard-ve-iv-mismatch.md) - price-to-beat and IV mismatch guards.
7. [senaryolar/05-ptb-bump-ve-max-price-relax.md](./senaryolar/05-ptb-bump-ve-max-price-relax.md) - PTB bump and max-price relaxation.
8. [senaryolar/06-tp-sl-time-exit-ve-reentry.md](./senaryolar/06-tp-sl-time-exit-ve-reentry.md) - take profit, stop loss, time exit, and re-entry.
9. [senaryolar/07-pair-lock-ve-edge-pairlock.md](./senaryolar/07-pair-lock-ve-edge-pairlock.md) - pair lock and edge pair-lock behavior.
10. [senaryolar/08-risk-guardlari-ve-hata-durumlari.md](./senaryolar/08-risk-guardlari-ve-hata-durumlari.md) - risk guards, retries, and blocked states.
11. [senaryolar/09-telegram-telemetri-ve-analiz.md](./senaryolar/09-telegram-telemetri-ve-analiz.md) - notifications, events, and analytics.
12. [senaryolar/10-volatility-capture-stratejileri.md](./senaryolar/10-volatility-capture-stratejileri.md) - volatility capture scenarios.
13. [senaryolar/11-dca-live-ve-trigger-binding.md](./senaryolar/11-dca-live-ve-trigger-binding.md) - DCA live action and trigger binding.
14. [senaryolar/12-adaptive-pair-lock-stratejileri.md](./senaryolar/12-adaptive-pair-lock-stratejileri.md) - adaptive pair-lock strategies.
15. [senaryolar/13-forensic-analiz-pnl-ve-decision-log.md](./senaryolar/13-forensic-analiz-pnl-ve-decision-log.md) - decision logs, snapshots, and PnL analysis.
16. [senaryolar/14-claim-sweep-ve-funds-activation.md](./senaryolar/14-claim-sweep-ve-funds-activation.md) - claim sweep and funds activation.

## Quick Map

| Need | Start here |
|---|---|
| Understand the flow builder nodes | [node-ozellikleri.md](./node-ozellikleri.md) |
| Debug missing orders | [senaryolar/08-risk-guardlari-ve-hata-durumlari.md](./senaryolar/08-risk-guardlari-ve-hata-durumlari.md) |
| Tune entry timing | [senaryolar/02-giris-trigger-ve-zamanlama.md](./senaryolar/02-giris-trigger-ve-zamanlama.md) |
| Tune PTB and IV edge behavior | [senaryolar/04-ptb-guard-ve-iv-mismatch.md](./senaryolar/04-ptb-guard-ve-iv-mismatch.md) |
| Configure pair-lock behavior | [senaryolar/07-pair-lock-ve-edge-pairlock.md](./senaryolar/07-pair-lock-ve-edge-pairlock.md) |
| Configure DCA live flows | [senaryolar/11-dca-live-ve-trigger-binding.md](./senaryolar/11-dca-live-ve-trigger-binding.md) |
| Read Telegram and event payloads | [senaryolar/09-telegram-telemetri-ve-analiz.md](./senaryolar/09-telegram-telemetri-ve-analiz.md) |
| Investigate a historical decision | [senaryolar/13-forensic-analiz-pnl-ve-decision-log.md](./senaryolar/13-forensic-analiz-pnl-ve-decision-log.md) |
| Troubleshoot claim or funds activation | [senaryolar/14-claim-sweep-ve-funds-activation.md](./senaryolar/14-claim-sweep-ve-funds-activation.md) |
| Use copyable configs | [ornekler/config-receteleri.md](./ornekler/config-receteleri.md) |

## Directory Layout

- [senaryolar/](./senaryolar/) - behavior-oriented feature guides.
- [referans/](./referans/) - node fields, outputs, events, and config references.
- [ornekler/](./ornekler/) - configuration recipes and troubleshooting checklists.

## How to Read a Feature

For each feature, identify:

1. The risk or workflow problem it addresses.
2. The config fields that enable it.
3. The telemetry or event names that confirm it ran.
4. The blocked, retry, or terminal states it can produce.
5. The related guards that can override or mask the same symptom.

This keeps debugging grounded in the order lifecycle instead of treating every missing fill, blocked guard, or absent notification as the same failure.
