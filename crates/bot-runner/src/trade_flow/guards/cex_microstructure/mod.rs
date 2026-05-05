mod binance;
mod coinbase;
mod service;
mod types;

#[allow(unused_imports)]
pub(crate) use service::{
    ensure_cex_microstructure_started, get_cex_current_price_snapshot,
    get_cex_microstructure_snapshot, CexMicrostructureSnapshotConfig,
};
#[allow(unused_imports)]
pub(crate) use types::{
    CexConsensusSnapshot, CexCurrentPriceSnapshot, CexImpulseSnapshot, CexSourceSnapshot, CexVenue,
    TakerSide,
};

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use service::{
    clear_cex_microstructure_test_state, seed_cex_book_test_sample, seed_cex_trade_test_sample,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use types::{CexBookSample, CexTradeSample};
