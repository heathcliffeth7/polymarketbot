use super::*;

#[test]
fn extract_price_midpoint_mode_prefers_best_bid_ask_over_price_changes() {
    let events = vec![WsEvent {
        channel: WsChannel::Market,
        payload: json!({
            "asset_id": "tok-yes",
            "best_bid": "0.57",
            "best_ask": "0.59",
            "price_changes": [
                { "asset_id": "tok-yes", "price": "0.14", "timestamp": 12345 }
            ]
        }),
        event_type: WsEventType::PriceChange,
        market: Some("tok-yes".to_string()),
        order_id: None,
        fill_id: None,
        status: None,
        price: None,
        size: None,
        ts: Some(12345),
    }];

    let raw = extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::Raw);
    assert_eq!(
        raw,
        Some(ExtractedWsPrice {
            price: 0.14,
            ts: Some(12345),
            source: "price_changes",
        })
    );

    let midpoint =
        extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::Midpoint);
    assert_eq!(
        midpoint,
        Some(ExtractedWsPrice {
            price: 0.58,
            ts: Some(12345),
            source: "best_bid_ask",
        })
    );
}

#[test]
fn ws_price_mode_parse_best_bid_ask_aliases() {
    assert_eq!(
        WsPriceMode::parse(Some("site_display"), WsPriceMode::Midpoint),
        WsPriceMode::SiteDisplay
    );
    assert_eq!(
        WsPriceMode::parse(Some("display"), WsPriceMode::Midpoint),
        WsPriceMode::SiteDisplay
    );
    assert_eq!(
        WsPriceMode::parse(Some("best_bid"), WsPriceMode::Midpoint),
        WsPriceMode::BestBid
    );
    assert_eq!(
        WsPriceMode::parse(Some("bid"), WsPriceMode::Midpoint),
        WsPriceMode::BestBid
    );
    assert_eq!(
        WsPriceMode::parse(Some("best_ask"), WsPriceMode::Midpoint),
        WsPriceMode::BestAsk
    );
    assert_eq!(
        WsPriceMode::parse(Some("ask"), WsPriceMode::Midpoint),
        WsPriceMode::BestAsk
    );
    assert_eq!(
        WsPriceMode::parse(Some("BEST_BID"), WsPriceMode::Midpoint),
        WsPriceMode::BestBid
    );
    assert_eq!(
        WsPriceMode::parse(Some(" Best_Ask "), WsPriceMode::Midpoint),
        WsPriceMode::BestAsk
    );
    assert_eq!(
        WsPriceMode::parse(Some("BID"), WsPriceMode::Midpoint),
        WsPriceMode::BestBid
    );
    assert_eq!(
        WsPriceMode::parse(Some("ASK"), WsPriceMode::Midpoint),
        WsPriceMode::BestAsk
    );
    assert_eq!(
        WsPriceMode::parse(Some("last_trade"), WsPriceMode::Midpoint),
        WsPriceMode::LastTrade
    );
    assert_eq!(
        WsPriceMode::parse(Some("last_trade_price"), WsPriceMode::Midpoint),
        WsPriceMode::LastTrade
    );
    assert_eq!(
        WsPriceMode::parse(Some("trade"), WsPriceMode::Midpoint),
        WsPriceMode::Raw
    );
}

#[test]
fn extract_price_best_bid_mode_returns_bid_only() {
    let events = vec![WsEvent {
        channel: WsChannel::Market,
        payload: json!({
            "asset_id": "tok-yes",
            "best_bid": "0.57",
            "best_ask": "0.59",
        }),
        event_type: WsEventType::PriceChange,
        market: Some("tok-yes".to_string()),
        order_id: None,
        fill_id: None,
        status: None,
        price: None,
        size: None,
        ts: Some(12345),
    }];

    let best_bid =
        extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::BestBid);
    assert_eq!(
        best_bid,
        Some(ExtractedWsPrice {
            price: 0.57,
            ts: Some(12345),
            source: "best_bid",
        })
    );
}

#[test]
fn extract_price_best_ask_mode_returns_ask_only() {
    let events = vec![WsEvent {
        channel: WsChannel::Market,
        payload: json!({
            "asset_id": "tok-yes",
            "best_bid": "0.57",
            "best_ask": "0.59",
        }),
        event_type: WsEventType::PriceChange,
        market: Some("tok-yes".to_string()),
        order_id: None,
        fill_id: None,
        status: None,
        price: None,
        size: None,
        ts: Some(12345),
    }];

    let best_ask =
        extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::BestAsk);
    assert_eq!(
        best_ask,
        Some(ExtractedWsPrice {
            price: 0.59,
            ts: Some(12345),
            source: "best_ask",
        })
    );
}

#[test]
fn extract_price_last_trade_mode_uses_trade_without_midpoint_fallback() {
    let direct_trade = vec![WsEvent {
        channel: WsChannel::Market,
        payload: json!({
            "asset_id": "tok-yes",
            "best_bid": "0.57",
            "best_ask": "0.59",
            "price": "0.61",
        }),
        event_type: WsEventType::PriceChange,
        market: Some("tok-yes".to_string()),
        order_id: None,
        fill_id: None,
        status: None,
        price: None,
        size: None,
        ts: Some(12345),
    }];

    let strict_last_trade = extract_price_from_market_events_with_mode(
        &direct_trade,
        "tok-yes",
        WsPriceMode::LastTrade,
    );
    assert_eq!(
        strict_last_trade,
        Some(ExtractedWsPrice {
            price: 0.61,
            ts: Some(12345),
            source: "payload_price",
        })
    );

    let no_trade = vec![WsEvent {
        channel: WsChannel::Market,
        payload: json!({
            "asset_id": "tok-yes",
            "best_bid": "0.57",
            "best_ask": "0.59",
        }),
        event_type: WsEventType::PriceChange,
        market: Some("tok-yes".to_string()),
        order_id: None,
        fill_id: None,
        status: None,
        price: None,
        size: None,
        ts: Some(12346),
    }];

    assert_eq!(
        extract_price_from_market_events_with_mode(&no_trade, "tok-yes", WsPriceMode::LastTrade),
        None
    );
    assert_eq!(
        extract_price_from_market_events_with_mode(&no_trade, "tok-yes", WsPriceMode::Raw),
        Some(ExtractedWsPrice {
            price: 0.58,
            ts: Some(12346),
            source: "best_bid_ask",
        })
    );
}

#[test]
fn extract_price_site_display_uses_midpoint_for_tight_spread() {
    let events = vec![WsEvent {
        channel: WsChannel::Market,
        payload: json!({
            "asset_id": "tok-yes",
            "best_bid": "0.77",
            "best_ask": "0.85",
            "price": "0.90",
        }),
        event_type: WsEventType::PriceChange,
        market: Some("tok-yes".to_string()),
        order_id: None,
        fill_id: None,
        status: None,
        price: None,
        size: None,
        ts: Some(12345),
    }];

    let site_display =
        extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::SiteDisplay);
    assert_eq!(
        site_display,
        Some(ExtractedWsPrice {
            price: 0.81,
            ts: Some(12345),
            source: "site_display_midpoint",
        })
    );
}

#[test]
fn extract_price_site_display_uses_last_trade_for_wide_spread() {
    let events = vec![WsEvent {
        channel: WsChannel::Market,
        payload: json!({
            "asset_id": "tok-yes",
            "best_bid": "0.70",
            "best_ask": "0.90",
            "price": "0.77",
        }),
        event_type: WsEventType::PriceChange,
        market: Some("tok-yes".to_string()),
        order_id: None,
        fill_id: None,
        status: None,
        price: None,
        size: None,
        ts: Some(12345),
    }];

    let site_display =
        extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::SiteDisplay);
    assert_eq!(
        site_display,
        Some(ExtractedWsPrice {
            price: 0.77,
            ts: Some(12345),
            source: "site_display_last_trade",
        })
    );
}

#[test]
fn extract_price_best_bid_from_price_changes() {
    let events = vec![WsEvent {
        channel: WsChannel::Market,
        payload: json!({
            "price_changes": [{
                "asset_id": "tok-yes",
                "best_bid": "0.45",
                "best_ask": "0.47",
                "timestamp": 99999
            }]
        }),
        event_type: WsEventType::PriceChange,
        market: None,
        order_id: None,
        fill_id: None,
        status: None,
        price: None,
        size: None,
        ts: Some(88888),
    }];

    let bid = extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::BestBid);
    assert_eq!(
        bid,
        Some(ExtractedWsPrice {
            price: 0.45,
            ts: Some(99999),
            source: "best_bid",
        })
    );

    let ask = extract_price_from_market_events_with_mode(&events, "tok-yes", WsPriceMode::BestAsk);
    assert_eq!(
        ask,
        Some(ExtractedWsPrice {
            price: 0.47,
            ts: Some(99999),
            source: "best_ask",
        })
    );
}
