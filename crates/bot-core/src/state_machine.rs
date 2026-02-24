use crate::types::TradeState;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransitionError {
    #[error("invalid state transition: {from:?} -> {to:?}")]
    Invalid { from: TradeState, to: TradeState },
}

pub fn can_transition(from: TradeState, to: TradeState) -> Result<(), TransitionError> {
    use TradeState::*;
    let valid = match (from, to) {
        (Idle, WaitingEntry)
        | (WaitingEntry, EntryPlaced)
        | (EntryPlaced, EntryPartiallyFilled)
        | (EntryPlaced, EntryFilled)
        | (EntryPartiallyFilled, EntryFilled)
        | (EntryFilled, TpPlaced)
        | (TpPlaced, SlArmed)
        | (TpPlaced, ExitFilled)
        | (SlArmed, ExitPartiallyFilled)
        | (SlArmed, ExitFilled)
        | (ExitPartiallyFilled, ExitFilled)
        | (ExitFilled, Settled)
        | (Settled, Idle)
        | (_, Halted) => true,
        _ => false,
    };

    if valid {
        Ok(())
    } else {
        Err(TransitionError::Invalid { from, to })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_valid_transition_path() {
        assert!(can_transition(TradeState::Idle, TradeState::WaitingEntry).is_ok());
        assert!(can_transition(TradeState::EntryFilled, TradeState::TpPlaced).is_ok());
        assert!(can_transition(TradeState::ExitFilled, TradeState::Settled).is_ok());
    }

    #[test]
    fn rejects_invalid_transition_path() {
        assert!(can_transition(TradeState::Idle, TradeState::TpPlaced).is_err());
        assert!(can_transition(TradeState::EntryPlaced, TradeState::Settled).is_err());
        assert!(can_transition(TradeState::Halted, TradeState::EntryPlaced).is_err());
    }
}
