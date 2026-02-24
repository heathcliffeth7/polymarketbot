'use client';

import { useState } from 'react';
import { PageShell } from '@/components/layout/page-shell';
import { OrderTable } from '@/components/orders/order-table';
import { FillTable } from '@/components/orders/fill-table';
import { Card, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Skeleton } from '@/components/ui/skeleton';
import { useOrders, useFills } from '@/hooks/use-orders';

export default function OrdersPage() {
  const [orderPage, setOrderPage] = useState(1);
  const [fillPage, setFillPage] = useState(1);
  const [statusFilter, setStatusFilter] = useState<string>('');
  const [intentFilter, setIntentFilter] = useState<string>('');

  const { data: ordersData, isLoading: ordersLoading } = useOrders(orderPage, 20, {
    status: statusFilter || undefined,
    intent: intentFilter || undefined,
  });
  const { data: fillsData, isLoading: fillsLoading } = useFills(fillPage, 20);

  return (
    <PageShell title="Orders & Fills">
      <Tabs defaultValue="orders" className="space-y-4">
        <TabsList className="bg-zinc-800">
          <TabsTrigger value="orders" className="data-[state=active]:bg-zinc-700">Orders</TabsTrigger>
          <TabsTrigger value="fills" className="data-[state=active]:bg-zinc-700">Fills</TabsTrigger>
        </TabsList>

        <TabsContent value="orders" className="space-y-4">
          <div className="flex items-center gap-4">
            <Select value={statusFilter} onValueChange={(v) => { setStatusFilter(v === 'all' ? '' : v); setOrderPage(1); }}>
              <SelectTrigger className="w-[180px] border-zinc-700 bg-zinc-800 text-zinc-200">
                <SelectValue placeholder="Status" />
              </SelectTrigger>
              <SelectContent className="border-zinc-700 bg-zinc-800">
                <SelectItem value="all" className="text-zinc-200">All Status</SelectItem>
                {['open', 'filled', 'partially_filled', 'canceled', 'rejected', 'expired', 'pending'].map((s) => (
                  <SelectItem key={s} value={s} className="text-zinc-200">{s}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Select value={intentFilter} onValueChange={(v) => { setIntentFilter(v === 'all' ? '' : v); setOrderPage(1); }}>
              <SelectTrigger className="w-[180px] border-zinc-700 bg-zinc-800 text-zinc-200">
                <SelectValue placeholder="Intent" />
              </SelectTrigger>
              <SelectContent className="border-zinc-700 bg-zinc-800">
                <SelectItem value="all" className="text-zinc-200">All Intents</SelectItem>
                {['entry', 'tp', 'sl', 'renewal'].map((i) => (
                  <SelectItem key={i} value={i} className="text-zinc-200">{i}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <Card className="border-zinc-800 bg-zinc-900">
            <CardContent className="p-0">
              {ordersLoading ? (
                <div className="space-y-2 p-4">
                  {Array.from({ length: 5 }).map((_, i) => (
                    <Skeleton key={i} className="h-10 bg-zinc-800" />
                  ))}
                </div>
              ) : (
                <OrderTable orders={ordersData?.data ?? []} />
              )}
            </CardContent>
          </Card>

          {ordersData && ordersData.totalPages > 1 && (
            <div className="flex items-center justify-center gap-2">
              <Button variant="outline" size="sm" onClick={() => setOrderPage((p) => Math.max(1, p - 1))} disabled={orderPage === 1} className="border-zinc-700 text-zinc-300">Previous</Button>
              <span className="text-sm text-zinc-400">Page {orderPage} of {ordersData.totalPages}</span>
              <Button variant="outline" size="sm" onClick={() => setOrderPage((p) => Math.min(ordersData.totalPages, p + 1))} disabled={orderPage === ordersData.totalPages} className="border-zinc-700 text-zinc-300">Next</Button>
            </div>
          )}
        </TabsContent>

        <TabsContent value="fills" className="space-y-4">
          <Card className="border-zinc-800 bg-zinc-900">
            <CardContent className="p-0">
              {fillsLoading ? (
                <div className="space-y-2 p-4">
                  {Array.from({ length: 5 }).map((_, i) => (
                    <Skeleton key={i} className="h-10 bg-zinc-800" />
                  ))}
                </div>
              ) : (
                <FillTable fills={fillsData?.data ?? []} />
              )}
            </CardContent>
          </Card>

          {fillsData && fillsData.totalPages > 1 && (
            <div className="flex items-center justify-center gap-2">
              <Button variant="outline" size="sm" onClick={() => setFillPage((p) => Math.max(1, p - 1))} disabled={fillPage === 1} className="border-zinc-700 text-zinc-300">Previous</Button>
              <span className="text-sm text-zinc-400">Page {fillPage} of {fillsData.totalPages}</span>
              <Button variant="outline" size="sm" onClick={() => setFillPage((p) => Math.min(fillsData.totalPages, p + 1))} disabled={fillPage === fillsData.totalPages} className="border-zinc-700 text-zinc-300">Next</Button>
            </div>
          )}
        </TabsContent>
      </Tabs>
    </PageShell>
  );
}
