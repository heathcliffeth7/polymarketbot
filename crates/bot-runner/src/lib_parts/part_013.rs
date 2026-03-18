#[allow(clippy::too_many_arguments)]
async fn execute_trade_flow_node(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    match node.node_type.as_str() {
        "trigger.market_price" => {
            execute_trigger_market_price(repo, cfg, client, ws, run, step, node, context).await
        }
        "trigger.sell_progress" => execute_trigger_sell_progress(repo, run, node, context).await,
        "trigger.open_positions" => {
            execute_trigger_open_positions(repo, client, ws, run, step, node, context).await
        }
        "trigger.position_drawdown" => {
            execute_trigger_position_drawdown(repo, ws, run, step, node, context).await
        }
        "trigger.time_window" => execute_trigger_time_window(node, context),
        "logic.if" => execute_logic_if(node, context),
        "logic.switch" => execute_logic_switch(node, context),
        "logic.delay" => execute_logic_delay(node),
        "logic.retry" => execute_logic_retry(node, step, context),
        "action.resolve_market" => execute_action_resolve_market(cfg, node, context).await,
        "action.dual_dca" => execute_action_dual_dca(repo, run, node, context).await,
        "action.place_order" => {
            execute_action_place_order(
                repo, run_id, cfg, limits, policy, run, step, node, graph, context,
            )
            .await
        }
        "action.cancel_order" => execute_action_cancel_order(repo, node, context).await,
        "action.update_order" => execute_action_update_order(repo, node, context).await,
        "action.set_state" => execute_action_set_state(node, context),
        "action.notify" => execute_action_notify(repo, run, node, context).await,
        "action.telegram_notify" => {
            execute_action_telegram_notify(repo, run, step, node, context).await
        }
        _ => Err(anyhow::anyhow!(
            "unsupported flow node type: {}",
            node.node_type
        )),
    }
}
