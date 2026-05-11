use super::*;

pub(crate) fn parse_state(raw: &str) -> Option<TradeState> {
    match raw {
        "Idle" => Some(TradeState::Idle),
        "WaitingEntry" => Some(TradeState::WaitingEntry),
        "EntryPlaced" => Some(TradeState::EntryPlaced),
        "EntryPartiallyFilled" => Some(TradeState::EntryPartiallyFilled),
        "EntryFilled" => Some(TradeState::EntryFilled),
        "TpPlaced" => Some(TradeState::TpPlaced),
        "SlArmed" => Some(TradeState::SlArmed),
        "ExitPartiallyFilled" => Some(TradeState::ExitPartiallyFilled),
        "ExitFilled" => Some(TradeState::ExitFilled),
        "Settled" => Some(TradeState::Settled),
        "Halted" => Some(TradeState::Halted),
        _ => None,
    }
}

pub(crate) fn leg_side_to_db(leg_side: LegSide) -> &'static str {
    match leg_side {
        LegSide::Yes => "yes",
        LegSide::No => "no",
    }
}

pub(crate) fn db_to_leg_side(raw: &str) -> Option<LegSide> {
    match raw {
        "yes" | "YES" => Some(LegSide::Yes),
        "no" | "NO" => Some(LegSide::No),
        _ => None,
    }
}
