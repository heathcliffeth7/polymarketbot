use super::iv_mismatch_edge_helpers::valid_probability;
use super::iv_mismatch_protection::PriceToBeatIvBookQuotes;

pub(crate) fn selected_book_mid_for_ptb_movement(
    book_quotes: PriceToBeatIvBookQuotes,
    selected_side: &str,
) -> Option<f64> {
    if selected_side == "up" {
        quote_mid_for_ptb_movement(book_quotes.up_bid, book_quotes.up_ask)
    } else {
        quote_mid_for_ptb_movement(book_quotes.down_bid, book_quotes.down_ask)
    }
}

fn quote_mid_for_ptb_movement(bid: Option<f64>, ask: Option<f64>) -> Option<f64> {
    let bid = bid.filter(|value| valid_probability(*value))?;
    let ask = ask.filter(|value| valid_probability(*value))?;
    (ask >= bid).then_some((bid + ask) / 2.0)
}
