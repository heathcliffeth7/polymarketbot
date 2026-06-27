use super::types::CexVenue;

pub(crate) fn active_anchor_venue_for_asset(asset: &str) -> CexVenue {
    match asset.trim().to_ascii_lowercase().as_str() {
        "sol" => CexVenue::Gateio,
        "hype" | "hyperliquid" => CexVenue::Coinbase,
        _ => CexVenue::Okx,
    }
}

pub(crate) fn active_spot_venues_for_asset(asset: &str) -> [CexVenue; 3] {
    [
        CexVenue::Binance,
        active_anchor_venue_for_asset(asset),
        CexVenue::Coinbase,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_venue_picks_gateio_for_sol() {
        assert_eq!(active_anchor_venue_for_asset("sol"), CexVenue::Gateio);
        assert_eq!(active_anchor_venue_for_asset("SOL"), CexVenue::Gateio);
    }

    #[test]
    fn anchor_venue_picks_coinbase_for_hype() {
        assert_eq!(active_anchor_venue_for_asset("hype"), CexVenue::Coinbase);
        assert_eq!(
            active_anchor_venue_for_asset("hyperliquid"),
            CexVenue::Coinbase
        );
    }

    #[test]
    fn anchor_venue_picks_okx_for_other_assets() {
        assert_eq!(active_anchor_venue_for_asset("btc"), CexVenue::Okx);
        assert_eq!(active_anchor_venue_for_asset("eth"), CexVenue::Okx);
        assert_eq!(active_anchor_venue_for_asset("xrp"), CexVenue::Okx);
        assert_eq!(active_anchor_venue_for_asset("doge"), CexVenue::Okx);
        assert_eq!(active_anchor_venue_for_asset("bnb"), CexVenue::Okx);
    }

    #[test]
    fn active_spot_venues_for_hype_includes_coinbase_anchor() {
        let venues = active_spot_venues_for_asset("hype");
        assert_eq!(venues[0], CexVenue::Binance);
        assert_eq!(venues[1], CexVenue::Coinbase);
        assert_eq!(venues[2], CexVenue::Coinbase);
    }
}
