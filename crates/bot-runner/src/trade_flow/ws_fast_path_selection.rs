fn select_ws_fast_path_targets(
    run_id: i64,
    cache: &TradeFlowWsFastPathCache,
    dirty_token_ids: Option<&[String]>,
) -> Vec<SelectedWsFastPathTarget> {
    match dirty_token_ids {
        Some(token_ids) => {
            let mut selected = Vec::new();
            let mut seen_targets = HashSet::new();
            let mut seen_market_pairs = HashSet::new();
            let mut dirty_markets = Vec::new();

            for dirty_token_id in token_ids {
                let Some(targets) = cache.token_targets.get(dirty_token_id.as_str()) else {
                    log_trigger_ws_dirty_token_unmapped(run_id, dirty_token_id);
                    continue;
                };

                for &(run_index, node_index) in targets {
                    let Some(node_spec) = cache
                        .run_specs
                        .get(run_index)
                        .and_then(|run_spec| run_spec.nodes.get(node_index))
                    else {
                        continue;
                    };

                    if seen_targets.insert((run_index, node_index)) {
                        selected.push(SelectedWsFastPathTarget {
                            run_index,
                            node_index,
                            dirty_token_id: Some(dirty_token_id.clone()),
                            reevaluation_reason: "dirty_token_match",
                        });
                    }

                    if let Some(market_slug) = node_spec.market_slug.as_ref() {
                        dirty_markets.push((market_slug.clone(), dirty_token_id.clone()));
                    }
                }
            }

            for (market_slug, dirty_token_id) in dirty_markets {
                if !seen_market_pairs.insert((market_slug.clone(), dirty_token_id.clone())) {
                    continue;
                }
                let Some(targets) = cache.market_targets.get(&market_slug) else {
                    continue;
                };
                for &(run_index, node_index) in targets {
                    if !seen_targets.insert((run_index, node_index)) {
                        continue;
                    }
                    selected.push(SelectedWsFastPathTarget {
                        run_index,
                        node_index,
                        dirty_token_id: Some(dirty_token_id.clone()),
                        reevaluation_reason: "market_dirty_fanout",
                    });
                }
            }

            selected
        }
        None => cache
            .run_specs
            .iter()
            .enumerate()
            .flat_map(|(run_index, run_spec)| {
                run_spec
                    .nodes
                    .iter()
                    .enumerate()
                    .map(move |(node_index, _)| SelectedWsFastPathTarget {
                        run_index,
                        node_index,
                        dirty_token_id: None,
                        reevaluation_reason: "full_refresh",
                    })
            })
            .collect(),
    }
}
