#[derive(Debug, Default)]
struct DeferredTradeBuilderSubmitEvents {
    guard_passed: Option<DeferredTradeBuilderGuardPassed>,
    order_events: Vec<DeferredTradeBuilderOrderEvent>,
}

#[derive(Debug)]
struct DeferredTradeBuilderGuardPassed {
    current_price: f64,
    desired_price: f64,
    best_ask: Option<f64>,
    trigger_price_guard: Value,
    execution_floor_guard: Value,
    max_price_guard: Value,
}

#[derive(Debug)]
struct DeferredTradeBuilderOrderEvent {
    event_type: &'static str,
    payload: Value,
}

impl DeferredTradeBuilderSubmitEvents {
    fn defer_guard_passed(
        &mut self,
        current_price: f64,
        desired_price: f64,
        best_ask: Option<f64>,
        trigger_price_guard: Value,
        execution_floor_guard: Value,
        max_price_guard: Value,
    ) {
        self.guard_passed = Some(DeferredTradeBuilderGuardPassed {
            current_price,
            desired_price,
            best_ask,
            trigger_price_guard,
            execution_floor_guard,
            max_price_guard,
        });
    }

    fn defer_order_event(&mut self, event_type: &'static str, payload: Value) {
        self.order_events.push(DeferredTradeBuilderOrderEvent {
            event_type,
            payload,
        });
    }

    async fn flush(&mut self, repo: &PostgresRepository, order: &TradeBuilderOrder) -> Result<()> {
        if let Some(event) = self.guard_passed.take() {
            append_trade_builder_guard_diagnostics_event(
                repo,
                order,
                event.current_price,
                event.desired_price,
                event.best_ask,
                event.trigger_price_guard,
                event.execution_floor_guard,
                event.max_price_guard,
                None,
                "passed",
                "guards_passed",
            )
            .await?;
        }

        for event in self.order_events.drain(..) {
            repo.append_trade_builder_order_event(order.id, event.event_type, &event.payload)
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod deferred_submit_event_tests {
    use super::*;

    #[test]
    fn deferred_submit_events_collects_guard_and_order_events() {
        let mut events = DeferredTradeBuilderSubmitEvents::default();
        events.defer_guard_passed(
            0.51,
            0.52,
            Some(0.53),
            json!({"pass": true}),
            json!({"pass": true}),
            json!({"pass": true}),
        );
        events.defer_order_event("optimistic_exit_submit_used", json!({"ok": true}));

        assert!(events.guard_passed.is_some());
        assert_eq!(events.order_events.len(), 1);
    }
}
