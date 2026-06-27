pub(super) const SUPPORTED_RTDS_SYMBOLS: &[&str] = &[
    "btc/usd", "eth/usd", "sol/usd", "xrp/usd", "doge/usd", "bnb/usd", "hype/usd",
];

pub(super) fn asset_to_symbol(asset: &str) -> Option<&'static str> {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" => Some("btc/usd"),
        "eth" => Some("eth/usd"),
        "sol" => Some("sol/usd"),
        "xrp" => Some("xrp/usd"),
        "doge" | "dogecoin" => Some("doge/usd"),
        "bnb" => Some("bnb/usd"),
        "hype" | "hyperliquid" => Some("hype/usd"),
        _ => None,
    }
}

pub(super) fn symbol_to_asset(symbol: &str) -> Option<&'static str> {
    match symbol.trim().to_ascii_lowercase().as_str() {
        "btc/usd" => Some("btc"),
        "eth/usd" => Some("eth"),
        "sol/usd" => Some("sol"),
        "xrp/usd" => Some("xrp"),
        "doge/usd" => Some("doge"),
        "bnb/usd" => Some("bnb"),
        "hype/usd" => Some("hype"),
        _ => None,
    }
}

pub(super) fn is_supported_symbol(symbol: &str) -> bool {
    SUPPORTED_RTDS_SYMBOLS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(symbol))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_to_symbol_maps_supported_assets() {
        assert_eq!(asset_to_symbol("btc"), Some("btc/usd"));
        assert_eq!(asset_to_symbol("ETH"), Some("eth/usd"));
        assert_eq!(asset_to_symbol(" sol "), Some("sol/usd"));
        assert_eq!(asset_to_symbol("xrp"), Some("xrp/usd"));
        assert_eq!(asset_to_symbol("doge"), Some("doge/usd"));
        assert_eq!(asset_to_symbol("bnb"), Some("bnb/usd"));
        assert_eq!(asset_to_symbol("hype"), Some("hype/usd"));
        assert_eq!(asset_to_symbol("hyperliquid"), Some("hype/usd"));
        assert_eq!(asset_to_symbol("dogecoin"), Some("doge/usd"));
    }

    #[test]
    fn symbol_to_asset_maps_supported_symbols() {
        assert_eq!(symbol_to_asset("BTC/USD"), Some("btc"));
        assert_eq!(symbol_to_asset("bnb/usd"), Some("bnb"));
        assert_eq!(symbol_to_asset("HYPE/USD"), Some("hype"));
        assert_eq!(symbol_to_asset("doge/usd"), Some("doge"));
    }

    #[test]
    fn is_supported_symbol_matches_rtds_symbol_list_case_insensitively() {
        assert!(is_supported_symbol("eth/usd"));
        assert!(is_supported_symbol("BTC/USD"));
        assert!(is_supported_symbol("bnb/usd"));
        assert!(is_supported_symbol("doge/usd"));
        assert!(is_supported_symbol("HYPE/USD"));
        assert!(!is_supported_symbol("ethUsd"));
        assert!(!is_supported_symbol("dogeusd"));
    }
}
