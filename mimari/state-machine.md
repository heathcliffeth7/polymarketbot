# Trade State Machine

## State Enum
- `Idle`
- `WaitingEntry`
- `EntryPlaced`
- `EntryPartiallyFilled`
- `EntryFilled`
- `TpPlaced`
- `SlArmed`
- `ExitPartiallyFilled`
- `ExitFilled`
- `Settled`
- `Halted`

## Transition Tablosu
1. `Idle -> WaitingEntry`: yeni market cycle başladı
2. `WaitingEntry -> EntryPlaced`: entry sinyali ve risk check allow
3. `EntryPlaced -> EntryPartiallyFilled`: ilk kısmi fill
4. `EntryPlaced -> EntryFilled`: tam fill
5. `EntryPartiallyFilled -> EntryFilled`: kalan fill tamamlandı
6. `EntryFilled -> TpPlaced`: TP order gönderildi
7. `TpPlaced -> ExitFilled`: TP fill oldu
8. `TpPlaced -> SlArmed`: fiyat aleyhe döndü, aggressive SL aktifleştirildi
9. `SlArmed -> ExitPartiallyFilled`: kısmi exit fill
10. `ExitPartiallyFilled -> ExitFilled`: tüm exit tamamlandı
11. `ExitFilled -> Settled`: PnL finalize edildi
12. `* -> Halted`: risk policy halt
13. `Settled -> Idle`: cycle kapandı

## Geçersiz Transitionlar
- `Idle -> TpPlaced`
- `EntryPlaced -> Settled`
- `Halted -> EntryPlaced`

## Recovery Kuralları
- Restart sonrası en son state DB'den okunur.
- Açık order varsa exchange status ile reconcile edilir.
- Reconcile bitmeden yeni entry yasak.

## Output/Log Gereksinimi
Her transition için log alanları:
- `trade_id`
- `market_id`
- `from_state`
- `to_state`
- `reason`
- `ts`
