use super::*;

pub(super) fn resolve_for_node(
    node: &TradeFlowNode,
    diagnostics: &Value,
) -> Option<PairLockPrimaryNotificationReason> {
    resolve_from_candidates(diagnostics, |candidate| {
        pair_lock_primary_notify_flag(node, candidate.scope)
    })
}

#[cfg(test)]
pub(super) fn resolve_without_node(
    diagnostics: &Value,
) -> Option<PairLockPrimaryNotificationReason> {
    resolve_from_candidates(diagnostics, |_| true)
}

fn resolve_from_candidates<F>(
    diagnostics: &Value,
    candidate_enabled: F,
) -> Option<PairLockPrimaryNotificationReason>
where
    F: Fn(&PairLockPrimaryNotificationReason) -> bool,
{
    let yes_candidate = diagnostics.get("yes_candidate_guard");
    let no_candidate = diagnostics.get("no_candidate_guard");
    let mut candidates = Vec::new();
    if let Some(candidate) =
        yes_candidate.and_then(pair_lock_primary_notification_reason_from_candidate)
    {
        candidates.push(candidate);
    }
    if let Some(candidate) =
        no_candidate.and_then(pair_lock_primary_notification_reason_from_candidate)
    {
        candidates.push(candidate);
    }
    let selected = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate_enabled(candidate))
        .min_by_key(|(_, candidate)| {
            (
                pair_lock_primary_notification_priority(candidate.scope),
                candidate.reason_code.clone(),
            )
        })
        .map(|(index, candidate)| (index, candidate.clone()))?;
    let secondary_candidate = candidates
        .into_iter()
        .enumerate()
        .find_map(|(index, candidate)| (index != selected.0).then_some(candidate.candidate));
    Some(PairLockPrimaryNotificationReason {
        secondary_candidate,
        ..selected.1
    })
}
