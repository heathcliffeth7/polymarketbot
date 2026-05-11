pub trait Strategy: Send + Sync {
    fn entry_signal(&self, current_price: f64, entry_price: f64) -> bool;
    fn take_profit_price(&self, entry_price: f64, tp_pct: f64) -> f64;
    fn aggressive_stop_price(&self, entry_price: f64, aggressive_sl_pct: f64) -> f64;
}

pub trait DualSideStrategy: Send + Sync {
    fn should_dca_leg(
        &self,
        current_price: f64,
        last_fill_price: Option<f64>,
        step_pct: f64,
        levels_filled: u32,
        max_levels: u32,
    ) -> bool;

    fn leg_take_profit_price(&self, avg_entry: f64, leg_tp_pct: f64) -> f64;

    fn should_flatten_basket(&self, basket_pnl_usdc: f64, tp_usdc: f64, sl_usdc: f64) -> bool;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PriceThresholdStrategy;

impl Strategy for PriceThresholdStrategy {
    fn entry_signal(&self, current_price: f64, entry_price: f64) -> bool {
        current_price >= entry_price
    }

    fn take_profit_price(&self, entry_price: f64, tp_pct: f64) -> f64 {
        (entry_price * (1.0 + tp_pct) * 100.0).round() / 100.0
    }

    fn aggressive_stop_price(&self, entry_price: f64, aggressive_sl_pct: f64) -> f64 {
        entry_price * (1.0 - aggressive_sl_pct)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SymmetricDualDcaStrategy;

impl DualSideStrategy for SymmetricDualDcaStrategy {
    fn should_dca_leg(
        &self,
        current_price: f64,
        last_fill_price: Option<f64>,
        step_pct: f64,
        levels_filled: u32,
        max_levels: u32,
    ) -> bool {
        if levels_filled >= max_levels {
            return false;
        }
        let Some(last_price) = last_fill_price else {
            return true;
        };
        if last_price <= 0.0 {
            return false;
        }
        ((current_price - last_price).abs() / last_price) >= step_pct
    }

    fn leg_take_profit_price(&self, avg_entry: f64, leg_tp_pct: f64) -> f64 {
        (avg_entry * (1.0 + leg_tp_pct) * 10000.0).round() / 10000.0
    }

    fn should_flatten_basket(&self, basket_pnl_usdc: f64, tp_usdc: f64, sl_usdc: f64) -> bool {
        basket_pnl_usdc >= tp_usdc || basket_pnl_usdc <= sl_usdc
    }
}

#[cfg(test)]
mod tests {
    use super::{DualSideStrategy, SymmetricDualDcaStrategy};

    #[test]
    fn dca_first_level_without_last_fill() {
        let strat = SymmetricDualDcaStrategy;
        assert!(strat.should_dca_leg(0.50, None, 0.02, 0, 3));
    }

    #[test]
    fn dca_requires_price_distance() {
        let strat = SymmetricDualDcaStrategy;
        assert!(!strat.should_dca_leg(0.501, Some(0.50), 0.02, 1, 3));
        assert!(strat.should_dca_leg(0.511, Some(0.50), 0.02, 1, 3));
    }

    #[test]
    fn dca_respects_max_levels() {
        let strat = SymmetricDualDcaStrategy;
        assert!(!strat.should_dca_leg(0.60, Some(0.50), 0.02, 3, 3));
    }

    #[test]
    fn basket_flatten_hits_tp_or_sl() {
        let strat = SymmetricDualDcaStrategy;
        assert!(strat.should_flatten_basket(0.40, 0.35, -0.60));
        assert!(strat.should_flatten_basket(-0.70, 0.35, -0.60));
        assert!(!strat.should_flatten_basket(0.10, 0.35, -0.60));
    }

    #[test]
    fn dca_max_levels_one() {
        let strat = SymmetricDualDcaStrategy;
        assert!(strat.should_dca_leg(0.50, None, 0.02, 0, 1));
        assert!(!strat.should_dca_leg(0.60, Some(0.50), 0.02, 1, 1));
    }

    #[test]
    fn dca_max_levels_five() {
        let strat = SymmetricDualDcaStrategy;
        assert!(strat.should_dca_leg(0.40, Some(0.50), 0.02, 4, 5));
        assert!(!strat.should_dca_leg(0.40, Some(0.50), 0.02, 5, 5));
    }

    #[test]
    fn dca_boundary_exact_step_distance() {
        let strat = SymmetricDualDcaStrategy;
        // (0.51 - 0.50) / 0.50 = 0.02 = step_pct → allow
        assert!(strat.should_dca_leg(0.51, Some(0.50), 0.02, 0, 3));
        // (0.5099 - 0.50) / 0.50 = 0.0198 < 0.02 → block
        assert!(!strat.should_dca_leg(0.5099, Some(0.50), 0.02, 1, 3));
    }

    #[test]
    fn dca_downward_price_also_triggers() {
        let strat = SymmetricDualDcaStrategy;
        // abs() yüzünden düşüş de tetikler
        assert!(strat.should_dca_leg(0.48, Some(0.50), 0.02, 1, 3));
        assert!(!strat.should_dca_leg(0.495, Some(0.50), 0.02, 1, 3));
    }

    #[test]
    fn dca_cap_simulation_max_3() {
        let strat = SymmetricDualDcaStrategy;
        let max = 3u32;
        let mut levels = 0u32;
        let mut last: Option<f64> = None;
        let steps = [0.50f64, 0.48, 0.46];
        for price in steps {
            assert!(strat.should_dca_leg(price, last, 0.02, levels, max));
            levels += 1;
            last = Some(price);
        }
        // 3/3 dolu → her fiyatta block
        assert!(!strat.should_dca_leg(0.44, last, 0.02, levels, max));
        assert!(!strat.should_dca_leg(0.01, last, 0.02, levels, max));
    }

    #[test]
    fn dca_cap_simulation_max_1() {
        let strat = SymmetricDualDcaStrategy;
        assert!(strat.should_dca_leg(0.50, None, 0.02, 0, 1));
        assert!(!strat.should_dca_leg(0.45, Some(0.50), 0.02, 1, 1));
    }

    #[test]
    fn dca_cap_simulation_max_5() {
        let strat = SymmetricDualDcaStrategy;
        let max = 5u32;
        let prices = [0.50f64, 0.48, 0.46, 0.44, 0.42];
        let mut levels = 0u32;
        let mut last: Option<f64> = None;
        for price in prices {
            assert!(strat.should_dca_leg(price, last, 0.02, levels, max));
            levels += 1;
            last = Some(price);
        }
        assert!(!strat.should_dca_leg(0.40, last, 0.02, levels, max));
    }
}
