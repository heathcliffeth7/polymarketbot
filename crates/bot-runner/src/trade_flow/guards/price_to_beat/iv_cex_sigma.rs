use super::iv_mismatch_math::root_mean_square;
use crate::trade_flow::guards::cex_microstructure::{
    active_spot_venues_for_asset, get_cex_book_samples, CexBookSample,
};
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

const CEX_SIGMA_MAX_BOOK_STALE_MS: i64 = 750;

#[cfg(test)]
static CEX_SIGMA_TEST_ENABLED: AtomicBool = AtomicBool::new(false);

pub(crate) fn cex_median_mid_sigma(asset: &str, fast_vol_window_secs: i64) -> Option<f64> {
    #[cfg(test)]
    if !CEX_SIGMA_TEST_ENABLED.load(Ordering::SeqCst) {
        return None;
    }

    let window_ms = fast_vol_window_secs.max(1) * 1_000;
    let mut sigmas = active_spot_venues_for_asset(asset)
        .into_iter()
        .filter_map(|venue| {
            get_cex_book_samples(asset, venue, window_ms, CEX_SIGMA_MAX_BOOK_STALE_MS)
                .ok()
                .and_then(|samples| venue_mid_sigma(&samples))
        })
        .collect::<Vec<_>>();
    median(&mut sigmas)
}

// CEX ve Chainlink sigma'yi agirlikli olarak birlestirir.
// Onceki `apply_cex_sigma_floor` koşulsuz `max(chainlink, cex)` seciyor ve SOL gibi
// trend'li CEX mid serilerinde sigma_eff'i sisiriyordu. Blend, iki kaynagi da sayar ama
// asiri yukari cekmeyi keser:
//   sigma_eff = sqrt(w_chainlink * chainlink^2 + w_cex * cex^2)
// Drift removal (venue_mid_sigma icinde) sayesinde cex_sigma zaten gercek vol'u yansitir.
pub(crate) fn blend_cex_sigma(
    existing_sigma_eff: f64,
    cex_sigma: Option<f64>,
    cex_blend_weight: f64,
) -> (f64, &'static str) {
    if let Some(cex_sigma) = cex_sigma.filter(|value| value.is_finite() && *value > 0.0) {
        let w_cex = cex_blend_weight.clamp(0.0, 1.0);
        if w_cex >= 1.0 {
            return (cex_sigma, "cex_median");
        }
        // w_cex=0 => blend katkisi sifir, saf chainlink source. Lojiğin net kalmasi icin
        // cex agirligiyla chainlink agirligini karsilastir: baskin kaynak source olur.
        let w_chainlink = 1.0 - w_cex;
        let cex_weighted = w_cex * cex_sigma * cex_sigma;
        let chainlink_weighted = w_chainlink * existing_sigma_eff * existing_sigma_eff;
        let blended = (chainlink_weighted + cex_weighted).sqrt();
        if blended.is_finite() && blended > 0.0 {
            let source = if cex_weighted >= chainlink_weighted {
                "cex_blend"
            } else {
                "chainlink_blend"
            };
            return (blended, source);
        }
    }
    (existing_sigma_eff, "chainlink")
}

fn venue_mid_sigma(samples: &[CexBookSample]) -> Option<f64> {
    let resampled = resample_mid_series_1s_last_tick(samples);
    let deltas = time_normalized_mid_deltas(&resampled);
    if deltas.len() < 2 {
        return None;
    }
    // Drift removal: delta serisinin ortalamasi (saniyelik drift) cikarilir, boylece
    // monoton ramp/tek yonlu hareket "volatilite" olarak sayilmaz. Gercek mean-reverting
    // bump'lar (gercek vol) korunur. 9 Haziran'daki standard_deviation davranisina paralel.
    let mean_delta = deltas.iter().sum::<f64>() / deltas.len() as f64;
    let centered: Vec<f64> = deltas.iter().map(|delta| delta - mean_delta).collect();
    let sigma = root_mean_square(&centered);
    (sigma.is_finite() && sigma > 0.0).then_some(sigma)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ResampledMid {
    timestamp_ms: i64,
    mid: f64,
}

fn resample_mid_series_1s_last_tick(samples: &[CexBookSample]) -> Vec<ResampledMid> {
    let mut ticks = samples
        .iter()
        .enumerate()
        .filter_map(|(index, sample)| {
            let mid = sample.mid();
            mid.is_finite().then_some((sample.timestamp_ms, index, mid))
        })
        .collect::<Vec<_>>();
    ticks.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

    let mut resampled: Vec<ResampledMid> = Vec::new();
    for (timestamp_ms, _, mid) in ticks {
        let bucket_start_ms = timestamp_ms.div_euclid(1_000) * 1_000;
        if let Some(last) = resampled.last_mut() {
            if last.timestamp_ms == bucket_start_ms {
                last.mid = mid;
                continue;
            }
        }
        resampled.push(ResampledMid {
            timestamp_ms: bucket_start_ms,
            mid,
        });
    }
    resampled
}

fn time_normalized_mid_deltas(samples: &[ResampledMid]) -> Vec<f64> {
    let mut deltas = Vec::new();
    for pair in samples.windows(2) {
        let prev = &pair[0];
        let next = &pair[1];
        let dt_secs = (next.timestamp_ms - prev.timestamp_ms) as f64 / 1_000.0;
        if dt_secs <= 0.0 {
            continue;
        }
        let delta = (next.mid - prev.mid) / dt_secs.sqrt();
        if delta.is_finite() {
            deltas.push(delta);
        }
    }
    deltas
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    Some(values[values.len() / 2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade_flow::guards::cex_microstructure::{
        clear_cex_microstructure_test_state, lock_cex_microstructure_test_state,
        seed_cex_book_test_sample, CexVenue,
    };
    use chrono::Utc;

    struct CexSigmaTestGuard(bool);

    impl CexSigmaTestGuard {
        fn enable() -> Self {
            Self(CEX_SIGMA_TEST_ENABLED.swap(true, Ordering::SeqCst))
        }
    }

    impl Drop for CexSigmaTestGuard {
        fn drop(&mut self) {
            CEX_SIGMA_TEST_ENABLED.store(self.0, Ordering::SeqCst);
        }
    }

    fn book(venue: CexVenue, ts: i64, mid: f64) -> CexBookSample {
        CexBookSample {
            venue,
            asset: "btc".to_string(),
            timestamp_ms: ts,
            bid: mid - 0.5,
            ask: mid + 0.5,
            bid_size: Some(1.0),
            ask_size: Some(1.0),
            source: "ticker",
        }
    }

    fn raw_time_normalized_mid_deltas(samples: &[CexBookSample]) -> Vec<f64> {
        let mut deltas = Vec::new();
        for pair in samples.windows(2) {
            let prev = &pair[0];
            let next = &pair[1];
            let dt_secs = (next.timestamp_ms - prev.timestamp_ms) as f64 / 1_000.0;
            if dt_secs <= 0.0 {
                continue;
            }
            let delta = (next.mid() - prev.mid()) / dt_secs.sqrt();
            if delta.is_finite() {
                deltas.push(delta);
            }
        }
        deltas
    }

    #[test]
    fn venue_mid_sigma_drift_removed_on_one_way_spike() {
        // Once `venue_mid_sigma_grows_on_one_way_spike`: tek yonlu hareket RMS ile
        // volatilite sayiliyordu. Drift removal icin sabit hizli monoton ramp kullaniriz:
        // 100 -> 103 -> 106 (her adim +3, yani pure drift, gercek vol yok).
        let samples = [
            book(CexVenue::Binance, 1_000, 100.0),
            book(CexVenue::Binance, 2_000, 103.0),
            book(CexVenue::Binance, 3_000, 106.0),
        ];

        let sigma = venue_mid_sigma(&samples);

        assert!(sigma.map_or(true, |value| value < 0.5), "drift sigma: {sigma:?}");
    }

    #[test]
    fn venue_mid_sigma_drift_removed_on_linear_trend() {
        // Once `one_second_resample_preserves_one_second_trend`: 100->101->102->103 lineer ramp
        // icin sigma==1.0 bekleniyordu (pure drift). Drift removal ile sigma ~0 olur.
        let samples = [
            book(CexVenue::Binance, 1_000, 100.0),
            book(CexVenue::Binance, 2_000, 101.0),
            book(CexVenue::Binance, 3_000, 102.0),
            book(CexVenue::Binance, 4_000, 103.0),
        ];

        let sigma = venue_mid_sigma(&samples);

        assert!(sigma.map_or(true, |value| value < 0.5), "linear drift sigma: {sigma:?}");
    }

    #[test]
    fn venue_mid_sigma_drift_removed_on_sparse_ramp() {
        // Once `one_second_resample_preserves_sparse_ticks`: 100->103->106 sparse drift.
        // Drift removal ile sigma ~0 olur.
        let samples = [
            book(CexVenue::Binance, 0, 100.0),
            book(CexVenue::Binance, 3_000, 103.0),
            book(CexVenue::Binance, 6_000, 106.0),
        ];

        let sigma = venue_mid_sigma(&samples);

        assert!(sigma.map_or(true, |value| value < 0.5), "sparse drift sigma: {sigma:?}");
    }

    #[test]
    fn one_second_resample_removes_subsecond_spread_bounce() {
        let samples = (0..30)
            .map(|index| {
                let mid = if index % 2 == 0 { 100.0 } else { 100.02 };
                book(CexVenue::Binance, index * 100, mid)
            })
            .collect::<Vec<_>>();

        let raw_sigma = root_mean_square(&raw_time_normalized_mid_deltas(&samples));

        assert!(raw_sigma > 0.05);
        assert_eq!(venue_mid_sigma(&samples), None);
    }

    #[test]
    fn venue_mid_sigma_preserves_real_volatility() {
        // Mean-reverting bump serisi: 100 -> 104 -> 100 -> 104. Burada gercek vol var,
        // drift ~0. Drift removal sonrasi sigma yuksek kalmali (gercek vol korunur).
        let samples = [
            book(CexVenue::Binance, 1_000, 100.0),
            book(CexVenue::Binance, 2_000, 104.0),
            book(CexVenue::Binance, 3_000, 100.0),
            book(CexVenue::Binance, 4_000, 104.0),
        ];

        let sigma = venue_mid_sigma(&samples).expect("sigma");

        assert!(sigma > 3.0, "mean-reverting vol korunmali: {sigma}");
    }

    #[test]
    fn median_filters_single_noisy_venue() {
        let mut values = [1.0, 1.2, 20.0];

        assert_eq!(median(&mut values), Some(1.2));
    }

    #[test]
    fn cex_median_sigma_returns_none_without_data() {
        let _guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();

        assert_eq!(cex_median_mid_sigma("btc", 15), None);
    }

    #[test]
    fn blend_cex_sigma_weights_sources() {
        // chainlink=0.038, cex=0.053, w_cex=0.4:
        //   sqrt(0.6 * 0.038^2 + 0.4 * 0.053^2)
        //   = sqrt(0.6*0.001444 + 0.4*0.002809)
        //   = sqrt(0.0008664 + 0.0011236)
        //   = sqrt(0.00199) ≈ 0.044609
        let (sigma, source) = blend_cex_sigma(0.038, Some(0.053), 0.4);
        assert!((sigma - 0.044609).abs() < 1e-4, "blended sigma: {sigma}");
        assert_eq!(source, "cex_blend");

        // cex < chainlink → chainlink_blend kaynagi, yine de iki kaynak karisik.
        // chainlink=0.053, cex=0.038, w_cex=0.4:
        //   sqrt(0.6*0.002809 + 0.4*0.001444) = sqrt(0.002263) ≈ 0.047571
        let (sigma_lower, source_lower) = blend_cex_sigma(0.053, Some(0.038), 0.4);
        assert!((sigma_lower - 0.047571).abs() < 1e-4, "blended sigma lower: {sigma_lower}");
        assert_eq!(source_lower, "chainlink_blend");
    }

    #[test]
    fn blend_cex_sigma_w_cex_one_uses_cex_directly() {
        // w_cex=1.0 → saf cex_median (eski floor davranisi), blend devre disi.
        let (sigma, source) = blend_cex_sigma(2.0, Some(3.0), 1.0);
        assert_eq!((sigma, source), (3.0, "cex_median"));
    }

    #[test]
    fn blend_cex_sigma_w_cex_zero_uses_chainlink() {
        // w_cex=0.0 → blend katkisi sifir, sigma=chainlink; baskin kaynak chainlink.
        let (sigma, source) = blend_cex_sigma(2.0, Some(3.0), 0.0);
        assert!((sigma - 2.0).abs() < 1e-9, "sigma: {sigma}");
        assert_eq!(source, "chainlink_blend");
    }

    #[test]
    fn blend_cex_sigma_falls_back_to_chainlink_when_cex_none() {
        let (sigma, source) = blend_cex_sigma(2.0, None, 0.4);
        assert_eq!((sigma, source), (2.0, "chainlink"));
    }

    #[test]
    fn blend_cex_sigma_falls_back_on_non_finite_cex() {
        let (sigma, source) = blend_cex_sigma(2.0, Some(f64::NAN), 0.4);
        assert_eq!((sigma, source), (2.0, "chainlink"));

        let (sigma_zero, source_zero) = blend_cex_sigma(2.0, Some(0.0), 0.4);
        assert_eq!((sigma_zero, source_zero), (2.0, "chainlink"));
    }

    #[test]
    fn cex_median_sigma_uses_spot_venue_median() {
        let _guard = lock_cex_microstructure_test_state();
        let _sigma_guard = CexSigmaTestGuard::enable();
        clear_cex_microstructure_test_state();
        let now = Utc::now().timestamp_millis();
        // Mean-reverting bump serileri (drift ~0), boylece drift removal gercek vol'u olcer.
        for (venue, mids) in [
            (CexVenue::Binance, [100.0, 103.0, 100.0]),
            (CexVenue::Okx, [100.0, 106.0, 100.0]),
            (CexVenue::Coinbase, [100.0, 104.0, 100.0]),
        ] {
            seed_cex_book_test_sample(book(venue, now - 2_000, mids[0]));
            seed_cex_book_test_sample(book(venue, now - 1_000, mids[1]));
            seed_cex_book_test_sample(book(venue, now, mids[2]));
        }

        let sigma = cex_median_mid_sigma("btc", 15).expect("sigma");

        assert!(sigma > 1.0, "median venue gercek vol'u olcmeli: {sigma}");
        assert!(sigma < 8.0);
    }

    #[test]
    fn sol_cex_median_sigma_uses_gateio_active_anchor() {
        let _guard = lock_cex_microstructure_test_state();
        let _sigma_guard = CexSigmaTestGuard::enable();
        clear_cex_microstructure_test_state();
        let now = Utc::now().timestamp_millis();
        // Mean-reverting bump serileri (drift ~0).
        for (venue, mids) in [
            (CexVenue::Binance, [100.0, 103.0, 100.0]),
            (CexVenue::Gateio, [100.0, 106.0, 100.0]),
            (CexVenue::Coinbase, [100.0, 104.0, 100.0]),
        ] {
            seed_cex_book_test_sample(book_for_asset("sol", venue, now - 2_000, mids[0]));
            seed_cex_book_test_sample(book_for_asset("sol", venue, now - 1_000, mids[1]));
            seed_cex_book_test_sample(book_for_asset("sol", venue, now, mids[2]));
        }

        let sigma = cex_median_mid_sigma("sol", 15).expect("sigma");

        assert!(sigma > 1.0, "SOL Gateio anchor gercek vol'u olcmeli: {sigma}");
        assert!(sigma < 8.0);
    }

    fn book_for_asset(asset: &str, venue: CexVenue, ts: i64, mid: f64) -> CexBookSample {
        CexBookSample {
            asset: asset.to_string(),
            ..book(venue, ts, mid)
        }
    }
}
