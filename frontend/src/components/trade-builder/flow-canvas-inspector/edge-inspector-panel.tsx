import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Separator } from '@/components/ui/separator';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import type { ConditionDraft } from '@/lib/trade-flow-config-mappers';
import { EDGE_TYPE_OPTIONS } from '../flow-canvas-constants';
import { GitBranch, Trash2 } from 'lucide-react';
import type { EdgeInspectorPanelProps } from './types';

export function EdgeInspectorPanel({
  edge,
  form,
  edgeTypeDraft,
  tab,
  actions,
}: EdgeInspectorPanelProps) {
  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 pb-1">
        <GitBranch className="h-4 w-4 text-sky-500" />
        <h3 className="text-sm font-semibold text-slate-800">Edge Ayarlari</h3>
      </div>
      <p className="text-[11px] text-slate-500">
        {edge.source} &rarr; {edge.target}
      </p>
      <Separator className="my-2" />

      <Tabs
        value={tab}
        onValueChange={(v) => actions.onTabChange(v as 'basic' | 'advanced')}
        className="flex min-h-0 flex-1 flex-col"
      >
        <TabsList className="bg-slate-100">
          <TabsTrigger value="basic">Form</TabsTrigger>
          <TabsTrigger value="advanced">Advanced</TabsTrigger>
        </TabsList>

        <div className="min-h-0 flex-1 overflow-y-auto">
          <TabsContent value="basic" className="space-y-3 pt-2">
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">Edge Type</Label>
              <Select value={edgeTypeDraft} onValueChange={(v) => actions.onEdgeTypeChange(v)}>
                <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {EDGE_TYPE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">Kosul Kullan</Label>
              <Select
                value={form.enabled ? 'yes' : 'no'}
                onValueChange={(v) =>
                  actions.onFormChange((prev) =>
                    prev ? { ...prev, enabled: v === 'yes' } : prev
                  )
                }
              >
                <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="no">Hayir</SelectItem>
                  <SelectItem value="yes">Evet</SelectItem>
                </SelectContent>
              </Select>
            </div>

            {form.enabled && (
              <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
                {!form.conditionSupported && (
                  <p className="text-[10px] text-amber-400">
                    Mevcut condition gelismis formatta. Form ile kaydedince simple condition formatina doner.
                  </p>
                )}
                <Input
                  value={form.conditionRow.leftVar}
                  onChange={(e) => actions.onUpdateConditionRow({ leftVar: e.target.value })}
                  placeholder="market_price"
                  className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                />
                <div className="grid grid-cols-3 gap-2">
                  <Select
                    value={form.conditionRow.operator}
                    onValueChange={(v) =>
                      actions.onUpdateConditionRow({ operator: v as ConditionDraft['operator'] })
                    }
                  >
                    <SelectTrigger className="h-8 border-slate-200 bg-white text-xs text-slate-900" size="sm">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value=">">&gt;</SelectItem>
                      <SelectItem value=">=">&gt;=</SelectItem>
                      <SelectItem value="<">&lt;</SelectItem>
                      <SelectItem value="<=">&lt;=</SelectItem>
                      <SelectItem value="==">==</SelectItem>
                      <SelectItem value="!=">!=</SelectItem>
                    </SelectContent>
                  </Select>
                  <Select
                    value={form.conditionRow.rightType}
                    onValueChange={(v) =>
                      actions.onUpdateConditionRow({ rightType: v as ConditionDraft['rightType'] })
                    }
                  >
                    <SelectTrigger className="h-8 border-slate-200 bg-white text-xs text-slate-900" size="sm">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="number">number</SelectItem>
                      <SelectItem value="string">string</SelectItem>
                      <SelectItem value="boolean">boolean</SelectItem>
                    </SelectContent>
                  </Select>
                  <Input
                    value={form.conditionRow.rightValue}
                    onChange={(e) => actions.onUpdateConditionRow({ rightValue: e.target.value })}
                    className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                  />
                </div>
              </div>
            )}
          </TabsContent>

          <TabsContent value="advanced" className="space-y-2 pt-2">
            <p className="text-[11px] text-amber-400">Gelismis mod condition JSON icindir.</p>
            <textarea
              value={form.advancedJson}
              onChange={(e) =>
                actions.onFormChange((prev) =>
                  prev ? { ...prev, advancedJson: e.target.value } : prev
                )
              }
              className="min-h-48 w-full rounded-md border border-slate-200 bg-white p-2 text-[11px] text-slate-900 focus-visible:ring-sky-300"
            />
          </TabsContent>
        </div>
      </Tabs>

      <Separator className="mt-2" />
      <div className="flex gap-2 pt-2">
        {tab === 'basic' ? (
          <Button size="sm" className="flex-1" onClick={actions.onApplyBasic}>
            Edge Uygula
          </Button>
        ) : (
          <Button size="sm" className="flex-1" onClick={actions.onApplyAdvanced}>
            JSON Uygula
          </Button>
        )}
        <Button
          size="sm"
          variant="outline"
          className="border-red-200 text-red-600 hover:bg-red-50 hover:text-red-700"
          onClick={actions.onDeleteEdge}
        >
          <Trash2 className="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>
  );
}
