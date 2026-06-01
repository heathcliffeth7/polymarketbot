import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { PTB_CURRENT_PRICE_SOURCE_OPTIONS } from "@/lib/trade-flow-config-mappers/ptb-modes";
import {
  POSITIVE_GRID_BASE_BUY_USDC_FIELD,
  POSITIVE_GRID_BLOCK_CONSECUTIVE_SAME_SIDE_BUYS_FIELD,
  POSITIVE_GRID_CYCLE_WINDOW_END_SEC_FIELD,
  POSITIVE_GRID_CYCLE_WINDOW_MODE_FIELD,
  POSITIVE_GRID_CYCLE_WINDOW_SECS_FIELD,
  POSITIVE_GRID_CYCLE_WINDOW_START_SEC_FIELD,
  POSITIVE_GRID_DEPTH_GUARD_FIELD,
  POSITIVE_GRID_EXECUTION_FLOOR_ENABLED_FIELD,
  POSITIVE_GRID_EXECUTION_FLOOR_PRICE_FIELD,
  POSITIVE_GRID_MIN_MARKETABLE_BUY_USDC_FIELD,
  POSITIVE_GRID_NO_BUY_RANGES_FIELD,
  POSITIVE_GRID_NORMAL_BUY_MAX_FIELD,
  POSITIVE_GRID_NORMAL_BUY_MIN_FIELD,
  POSITIVE_GRID_PARTIAL_RECOVERY_BALANCE_RESERVE_FIELD,
  POSITIVE_GRID_PARTIAL_RECOVERY_ENABLED_FIELD,
  POSITIVE_GRID_PARTIAL_RECOVERY_IGNORE_MARKET_BUDGET_FIELD,
  POSITIVE_GRID_PARTIAL_RECOVERY_MAX_BUY_FIELD,
  POSITIVE_GRID_PARTIAL_RECOVERY_MIN_LOSS_REDUCTION_FIELD,
  POSITIVE_GRID_INVENTORY_BALANCE_LEAD_QTY_FIELD,
  POSITIVE_GRID_PTB_CURRENT_SOURCE_FIELD,
  POSITIVE_GRID_PTB_DIFF_UNIT_FIELD,
  POSITIVE_GRID_PTB_GUARD_FIELD,
  POSITIVE_GRID_PTB_MIN_DIFF_FIELD,
  POSITIVE_GRID_PTB_RESCUE_MIN_DIFF_FIELD,
  POSITIVE_GRID_PROFIT_TARGET_FIELD,
  POSITIVE_GRID_QUANTITY_SIZING_MODE_FIELD,
  POSITIVE_GRID_RESCUE_BUY_ENABLED_FIELD,
  POSITIVE_GRID_RESCUE_MIN_PRICE_FIELD,
  POSITIVE_GRID_RESCUE_MAX_PRICE_FIELD,
  POSITIVE_GRID_SIZING_PRICE_BUFFER_FIELD,
  POSITIVE_GRID_STOP_BUYS_AFTER_PAIRLOCK_MERGE_FIELD,
  POSITIVE_GRID_TAKE_PROFIT_SELL_BID_FIELD,
  POSITIVE_GRID_TRIGGER_PRICE_GUARD_FIELD,
} from "@/lib/trade-flow-config-mappers/positive-quantity-flip-grid";

interface PositiveGridSectionProps {
  visible: boolean;
  pairlockCompressionMode?: boolean;
  fields: Record<string, string>;
  onUpdateField: (key: string, value: string) => void;
}

export function PositiveGridSection({
  visible,
  pairlockCompressionMode = false,
  fields,
  onUpdateField,
}: PositiveGridSectionProps) {
  if (!visible) return null;
  const boolValue = (key: string, fallback: boolean) =>
    (fields[key] ?? (fallback ? "true" : "false")) === "true";
  const updateBool = (key: string, checked: boolean) =>
    onUpdateField(key, checked ? "true" : "false");
  const quantitySizingMode =
    fields[POSITIVE_GRID_QUANTITY_SIZING_MODE_FIELD] || "profit_target";
  const fixedUsdcSizingEnabled = quantitySizingMode === "fixed_usdc";
  const cycleWindowMode =
    fields[POSITIVE_GRID_CYCLE_WINDOW_MODE_FIELD] || "custom_range";
  const rescueBuyEnabled = boolValue(POSITIVE_GRID_RESCUE_BUY_ENABLED_FIELD, false);
  const ptbGuardEnabled = boolValue(POSITIVE_GRID_PTB_GUARD_FIELD, false);

  return (
    <div className="space-y-2 rounded-md border border-emerald-200/80 bg-emerald-50/80 p-3">
      <div className="text-[11px] font-semibold uppercase tracking-wide text-emerald-700">
        Cycle Window
      </div>
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">Mode</Label>
          <Select
            value={cycleWindowMode}
            onValueChange={(value) =>
              onUpdateField(POSITIVE_GRID_CYCLE_WINDOW_MODE_FIELD, value)
            }
          >
            <SelectTrigger
              className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900"
              size="sm"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="off">Off</SelectItem>
              <SelectItem value="first">First N seconds</SelectItem>
              <SelectItem value="last">Last N seconds</SelectItem>
              <SelectItem value="custom_range">Custom range</SelectItem>
            </SelectContent>
          </Select>
        </div>
        {(cycleWindowMode === "first" || cycleWindowMode === "last") && (
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">
              Window Seconds
            </Label>
            <Input
              type="number"
              min={1}
              max={900}
              step={1}
              value={fields[POSITIVE_GRID_CYCLE_WINDOW_SECS_FIELD] || "120"}
              onChange={(event) =>
                onUpdateField(
                  POSITIVE_GRID_CYCLE_WINDOW_SECS_FIELD,
                  event.target.value,
                )
              }
              className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
            />
          </div>
        )}
        {cycleWindowMode === "custom_range" && (
          <>
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">
                Start Sec
              </Label>
              <Input
                type="number"
                min={0}
                max={900}
                step={1}
                value={
                  fields[POSITIVE_GRID_CYCLE_WINDOW_START_SEC_FIELD] || "0"
                }
                onChange={(event) =>
                  onUpdateField(
                    POSITIVE_GRID_CYCLE_WINDOW_START_SEC_FIELD,
                    event.target.value,
                  )
                }
                className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
              />
            </div>
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">
                End Sec
              </Label>
              <Input
                type="number"
                min={1}
                max={900}
                step={1}
                value={
                  fields[POSITIVE_GRID_CYCLE_WINDOW_END_SEC_FIELD] || "300"
                }
                onChange={(event) =>
                  onUpdateField(
                    POSITIVE_GRID_CYCLE_WINDOW_END_SEC_FIELD,
                    event.target.value,
                  )
                }
                className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
              />
            </div>
          </>
        )}
      </div>
      <div className="space-y-1">
        <p className="text-[10px] leading-relaxed text-slate-500">
          Start/End saniyeleri cycle baslangicindan sayilir. Son 3 dakika icin
          120-300, son 2 dakika icin 180-300 gir.
        </p>
      </div>
      <div className="text-[11px] font-semibold uppercase tracking-wide text-emerald-700">
        Trade Guards
      </div>
      <div className="grid grid-cols-1 gap-2">
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">
              Normal Buy Min (cent)
            </Label>
            <Input
              type="number"
              min={1}
              max={99}
              step={0.1}
              value={fields[POSITIVE_GRID_NORMAL_BUY_MIN_FIELD] || "50"}
              onChange={(event) =>
                onUpdateField(
                  POSITIVE_GRID_NORMAL_BUY_MIN_FIELD,
                  event.target.value,
                )
              }
              className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">
              Normal Buy Max (cent)
            </Label>
            <Input
              type="number"
              min={1}
              max={100}
              step={0.1}
              value={fields[POSITIVE_GRID_NORMAL_BUY_MAX_FIELD] || "60"}
              onChange={(event) =>
                onUpdateField(
                  POSITIVE_GRID_NORMAL_BUY_MAX_FIELD,
                  event.target.value,
                )
              }
              className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
            />
          </div>
          <p className="text-[10px] leading-relaxed text-slate-500 sm:col-span-2">
            Normal max hem entryBandMaxCent hem hardMaxPriceCent hem
            worstPriceCent olarak kaydedilir.
          </p>
        </div>
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">
              {pairlockCompressionMode && fixedUsdcSizingEnabled
                ? "Flip Başına USDC"
                : pairlockCompressionMode
                  ? "İlk Alım USDC"
                  : "Normal Buy USDC"}
            </Label>
            <Input
              type="number"
              min={1.05}
              step={0.01}
              value={
                fields[POSITIVE_GRID_BASE_BUY_USDC_FIELD] ||
                (pairlockCompressionMode ? "2" : "1")
              }
              onChange={(event) =>
                onUpdateField(
                  POSITIVE_GRID_BASE_BUY_USDC_FIELD,
                  event.target.value,
                )
              }
              className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
            />
            {pairlockCompressionMode && fixedUsdcSizingEnabled ? (
              <p className="text-[10px] leading-relaxed text-slate-500">
                Her grid alımı bu tutarı kullanır (ör. 2, 5 veya 10 USDC). Bakiye
                arttıkça buradan yükseltebilirsin.
              </p>
            ) : pairlockCompressionMode ? (
              <p className="text-[10px] leading-relaxed text-slate-500">
                İlk grid alımının hedef tutarı; compression buy bu alanı etkilemez.
              </p>
            ) : null}
          </div>
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">
              Normal Min Marketable BUY USDC
            </Label>
            <Input
              type="number"
              min={1}
              step={0.01}
              value={
                fields[POSITIVE_GRID_MIN_MARKETABLE_BUY_USDC_FIELD] || "1.05"
              }
              onChange={(event) =>
                onUpdateField(
                  POSITIVE_GRID_MIN_MARKETABLE_BUY_USDC_FIELD,
                  event.target.value,
                )
              }
              className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
            />
          </div>
        </div>
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">
            Quantity Profit Target (USDC)
          </Label>
          <Input
            type="number"
            min={0.01}
            step={0.01}
            value={fields[POSITIVE_GRID_PROFIT_TARGET_FIELD] || "1"}
            onChange={(event) =>
              onUpdateField(
                POSITIVE_GRID_PROFIT_TARGET_FIELD,
                event.target.value,
              )
            }
            className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
          />
          <p className="text-[10px] leading-relaxed text-slate-500">
            Bu hedef hem buy quantity hesabinda hem 98c sell filtresinde
            kullanilir.
          </p>
        </div>
        <div className="space-y-2 rounded border border-emerald-200 bg-white/70 p-2">
          <div className="text-[11px] font-semibold uppercase tracking-wide text-emerald-700">
            Take Profit
          </div>
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">
                Take Profit Sell Bid (cent)
              </Label>
              <Input
                type="number"
                min={1}
                max={100}
                step={0.1}
                value={fields[POSITIVE_GRID_TAKE_PROFIT_SELL_BID_FIELD] || "98"}
                onChange={(event) =>
                  onUpdateField(
                    POSITIVE_GRID_TAKE_PROFIT_SELL_BID_FIELD,
                    event.target.value,
                  )
                }
                className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
              />
            </div>
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">
                Sell Profit Target (USDC)
              </Label>
              <Input
                type="number"
                min={0.01}
                step={0.01}
                value={fields[POSITIVE_GRID_PROFIT_TARGET_FIELD] || "1"}
                onChange={(event) =>
                  onUpdateField(
                    POSITIVE_GRID_PROFIT_TARGET_FIELD,
                    event.target.value,
                  )
                }
                className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
              />
            </div>
          </div>
          <p className="text-[10px] leading-relaxed text-slate-500">
            UP veya DOWN bid bu seviyeye gelirse, toplam net kar hedefi
            saglaniyorsa full sell yapilir.
          </p>
        </div>
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">
            Sizing Fill Buffer (cent)
          </Label>
          <Input
            type="number"
            min={0}
            max={5}
            step={0.1}
            value={fields[POSITIVE_GRID_SIZING_PRICE_BUFFER_FIELD] || "3"}
            onChange={(event) =>
              onUpdateField(
                POSITIVE_GRID_SIZING_PRICE_BUFFER_FIELD,
                event.target.value,
              )
            }
            className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
          />
          <p className="text-[10px] leading-relaxed text-slate-500">
            Quantity hesabi best ask uzerine bu buffer eklenerek yapilir. 3 = 3c
            kotu fill varsay.
          </p>
        </div>
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">
              Quantity Sizing Mode
            </Label>
            <Select
              value={
                fixedUsdcSizingEnabled
                  ? "profit_target"
                  : quantitySizingMode
              }
              disabled={fixedUsdcSizingEnabled}
              onValueChange={(value) =>
                onUpdateField(POSITIVE_GRID_QUANTITY_SIZING_MODE_FIELD, value)
              }
            >
              <SelectTrigger
                className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900"
                size="sm"
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="profit_target">Profit target</SelectItem>
                <SelectItem value="inventory_balance">
                  Inventory balance
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">
              Inventory Lead Qty
            </Label>
            <Input
              type="number"
              min={0}
              max={1000}
              step={0.01}
              value={
                fields[POSITIVE_GRID_INVENTORY_BALANCE_LEAD_QTY_FIELD] || "0"
              }
              onChange={(event) =>
                onUpdateField(
                  POSITIVE_GRID_INVENTORY_BALANCE_LEAD_QTY_FIELD,
                  event.target.value,
                )
              }
              className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
            />
          </div>
          <p className="text-[10px] leading-relaxed text-slate-500 sm:col-span-2">
            Inventory balance modu, profit hedefi icin buy buyutmek yerine karsi
            taraf quantity farkini butce cap icinde azaltir.
            {fixedUsdcSizingEnabled
              ? " Fixed USDC flip sizing acikken bu secim devre disidir."
              : null}
          </p>
        </div>
        <div className="space-y-2 rounded-md border border-emerald-100 bg-white/60 p-2">
          <div className="text-[11px] font-semibold uppercase tracking-wide text-emerald-700">
            Partial Recovery
          </div>
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            <GuardSwitch
              label="Partial recovery"
              checked={boolValue(
                POSITIVE_GRID_PARTIAL_RECOVERY_ENABLED_FIELD,
                false,
              )}
              onCheckedChange={(checked) =>
                updateBool(POSITIVE_GRID_PARTIAL_RECOVERY_ENABLED_FIELD, checked)
              }
            />
            <GuardSwitch
              label="Ignore market budget"
              checked={boolValue(
                POSITIVE_GRID_PARTIAL_RECOVERY_IGNORE_MARKET_BUDGET_FIELD,
                true,
              )}
              onCheckedChange={(checked) =>
                updateBool(
                  POSITIVE_GRID_PARTIAL_RECOVERY_IGNORE_MARKET_BUDGET_FIELD,
                  checked,
                )
              }
            />
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">
                Min Loss Reduction (USDC)
              </Label>
              <Input
                type="number"
                min={0}
                step={0.01}
                value={
                  fields[
                    POSITIVE_GRID_PARTIAL_RECOVERY_MIN_LOSS_REDUCTION_FIELD
                  ] || "0.1"
                }
                onChange={(event) =>
                  onUpdateField(
                    POSITIVE_GRID_PARTIAL_RECOVERY_MIN_LOSS_REDUCTION_FIELD,
                    event.target.value,
                  )
                }
                className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
              />
            </div>
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">
                Balance Reserve (USDC)
              </Label>
              <Input
                type="number"
                min={0}
                step={0.01}
                value={
                  fields[POSITIVE_GRID_PARTIAL_RECOVERY_BALANCE_RESERVE_FIELD] ||
                  "1"
                }
                onChange={(event) =>
                  onUpdateField(
                    POSITIVE_GRID_PARTIAL_RECOVERY_BALANCE_RESERVE_FIELD,
                    event.target.value,
                  )
                }
                className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
              />
            </div>
            <div className="space-y-1 sm:col-span-2">
              <Label className="text-[11px] font-medium text-slate-600">
                Max Partial Buy (USDC)
              </Label>
              <Input
                type="number"
                min={0}
                step={0.01}
                value={fields[POSITIVE_GRID_PARTIAL_RECOVERY_MAX_BUY_FIELD] || ""}
                onChange={(event) =>
                  onUpdateField(
                    POSITIVE_GRID_PARTIAL_RECOVERY_MAX_BUY_FIELD,
                    event.target.value,
                  )
                }
                placeholder="No fixed cap"
                className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
              />
            </div>
          </div>
        </div>
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
          <GuardSwitch
            label="Depth guard"
            checked={boolValue(POSITIVE_GRID_DEPTH_GUARD_FIELD, true)}
            onCheckedChange={(checked) =>
              updateBool(POSITIVE_GRID_DEPTH_GUARD_FIELD, checked)
            }
          />
          <GuardSwitch
            label="Trigger price guard"
            checked={boolValue(POSITIVE_GRID_TRIGGER_PRICE_GUARD_FIELD, false)}
            onCheckedChange={(checked) =>
              updateBool(POSITIVE_GRID_TRIGGER_PRICE_GUARD_FIELD, checked)
            }
          />
          <GuardSwitch
            label="Block same-side repeat"
            checked={boolValue(
              POSITIVE_GRID_BLOCK_CONSECUTIVE_SAME_SIDE_BUYS_FIELD,
              true,
            )}
            onCheckedChange={(checked) =>
              updateBool(
                POSITIVE_GRID_BLOCK_CONSECUTIVE_SAME_SIDE_BUYS_FIELD,
                checked,
              )
            }
          />
          {pairlockCompressionMode ? (
            <>
              <GuardSwitch
                label="Fixed USDC flip sizing"
                checked={fixedUsdcSizingEnabled}
                onCheckedChange={(checked) =>
                  onUpdateField(
                    POSITIVE_GRID_QUANTITY_SIZING_MODE_FIELD,
                    checked ? "fixed_usdc" : "profit_target",
                  )
                }
              />
              <GuardSwitch
                label="Stop buys after pairlock merge"
                checked={boolValue(
                  POSITIVE_GRID_STOP_BUYS_AFTER_PAIRLOCK_MERGE_FIELD,
                  true,
                )}
                onCheckedChange={(checked) =>
                  updateBool(
                    POSITIVE_GRID_STOP_BUYS_AFTER_PAIRLOCK_MERGE_FIELD,
                    checked,
                  )
                }
              />
            </>
          ) : null}
          {pairlockCompressionMode ? (
            <p className="text-[10px] leading-relaxed text-slate-500">
              Fixed USDC flip sizing acikken her flip sabit baseBuyUsdc kullanir;
              net_cost&apos;a gore qty buyumez. Kapatinca profit_target&apos;a
              doner.
            </p>
          ) : null}
        </div>
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-[1fr_1fr_1fr]">
          <GuardSwitch
            label="Rescue buy"
            checked={boolValue(POSITIVE_GRID_RESCUE_BUY_ENABLED_FIELD, false)}
            onCheckedChange={(checked) =>
              updateBool(POSITIVE_GRID_RESCUE_BUY_ENABLED_FIELD, checked)
            }
          />
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">
              Rescue Min Price (cent)
            </Label>
            <Input
              type="number"
              min={1}
              max={97}
              step={0.1}
              value={fields[POSITIVE_GRID_RESCUE_MIN_PRICE_FIELD] || "60"}
              onChange={(event) =>
                onUpdateField(
                  POSITIVE_GRID_RESCUE_MIN_PRICE_FIELD,
                  event.target.value,
                )
              }
              className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">
              Rescue Max Price (cent)
            </Label>
            <Input
              type="number"
              min={1}
              max={97}
              step={0.1}
              value={fields[POSITIVE_GRID_RESCUE_MAX_PRICE_FIELD] || "70"}
              onChange={(event) =>
                onUpdateField(
                  POSITIVE_GRID_RESCUE_MAX_PRICE_FIELD,
                  event.target.value,
                )
              }
              className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
            />
          </div>
        </div>
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-[1fr_1fr]">
          <GuardSwitch
            label="Execution floor"
            checked={boolValue(
              POSITIVE_GRID_EXECUTION_FLOOR_ENABLED_FIELD,
              true,
            )}
            onCheckedChange={(checked) =>
              updateBool(POSITIVE_GRID_EXECUTION_FLOOR_ENABLED_FIELD, checked)
            }
          />
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">
              Execution Floor Price (cent)
            </Label>
            <Input
              type="number"
              min={1}
              max={100}
              step={0.1}
              value={fields[POSITIVE_GRID_EXECUTION_FLOOR_PRICE_FIELD] || ""}
              onChange={(event) =>
                onUpdateField(
                  POSITIVE_GRID_EXECUTION_FLOOR_PRICE_FIELD,
                  event.target.value,
                )
              }
              placeholder="Entry min kullan"
              className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
            />
          </div>
        </div>
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">
            Positive Grid No-Buy Ranges
          </Label>
          <textarea
            value={fields[POSITIVE_GRID_NO_BUY_RANGES_FIELD] || "[]"}
            onChange={(event) =>
              onUpdateField(
                POSITIVE_GRID_NO_BUY_RANGES_FIELD,
                event.target.value,
              )
            }
            placeholder='[{"minCent":56,"maxCent":60}]'
            className="min-h-20 w-full rounded-md border border-slate-200 bg-white p-2 font-mono text-[11px] text-slate-900 focus-visible:ring-emerald-300"
          />
          <p className="text-[10px] leading-relaxed text-slate-500">
            Inclusive araliklar: ask minCent ve maxCent arasindaysa buy
            candidate acilmaz.
          </p>
        </div>
        <div className="space-y-2 rounded-md border border-emerald-100 bg-white/60 p-2">
          <GuardSwitch
            label="PTB guard"
            checked={boolValue(POSITIVE_GRID_PTB_GUARD_FIELD, false)}
            onCheckedChange={(checked) =>
              updateBool(POSITIVE_GRID_PTB_GUARD_FIELD, checked)
            }
          />
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">
                Entry PTB Min Diff (Normal Buy)
              </Label>
              <Input
                type="number"
                min={0}
                step={0.1}
                value={fields[POSITIVE_GRID_PTB_MIN_DIFF_FIELD] || "2"}
                onChange={(event) =>
                  onUpdateField(
                    POSITIVE_GRID_PTB_MIN_DIFF_FIELD,
                    event.target.value,
                  )
                }
                className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
              />
            </div>
            {rescueBuyEnabled && ptbGuardEnabled ? (
              <div className="space-y-1">
                <Label className="text-[11px] font-medium text-slate-600">
                  Entry PTB Min Diff (Rescue Buy)
                </Label>
                <Input
                  type="number"
                  min={0}
                  step={0.1}
                  value={fields[POSITIVE_GRID_PTB_RESCUE_MIN_DIFF_FIELD] || ""}
                  onChange={(event) =>
                    onUpdateField(
                      POSITIVE_GRID_PTB_RESCUE_MIN_DIFF_FIELD,
                      event.target.value,
                    )
                  }
                  placeholder={fields[POSITIVE_GRID_PTB_MIN_DIFF_FIELD] || "2"}
                  className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-emerald-300"
                />
              </div>
            ) : null}
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">
                PTB Unit
              </Label>
              <Select
                value={fields[POSITIVE_GRID_PTB_DIFF_UNIT_FIELD] || "usd"}
                onValueChange={(value) =>
                  onUpdateField(POSITIVE_GRID_PTB_DIFF_UNIT_FIELD, value)
                }
              >
                <SelectTrigger
                  className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900"
                  size="sm"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="usd">USD</SelectItem>
                  <SelectItem value="cent">Cent</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">
                PTB Source
              </Label>
              <Select
                value={
                  fields[POSITIVE_GRID_PTB_CURRENT_SOURCE_FIELD] || "chainlink"
                }
                onValueChange={(value) =>
                  onUpdateField(POSITIVE_GRID_PTB_CURRENT_SOURCE_FIELD, value)
                }
              >
                <SelectTrigger
                  className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900"
                  size="sm"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {PTB_CURRENT_PRICE_SOURCE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
          {rescueBuyEnabled && ptbGuardEnabled ? (
            <p className="text-[10px] leading-relaxed text-slate-500">
              Rescue entry PTB bos birakilirsa normal entry PTB kullanilir.
              Ornek: normal 80, rescue 40 - kismi donuste hedge icin daha
              dusuk esik.
            </p>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function GuardSwitch({
  label,
  checked,
  onCheckedChange,
}: {
  label: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-md border border-slate-200 bg-white px-2 py-1.5">
      <Label className="text-[11px] font-medium text-slate-600">{label}</Label>
      <Switch size="sm" checked={checked} onCheckedChange={onCheckedChange} />
    </div>
  );
}
