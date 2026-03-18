struct RunnerProcessLock {
    path: PathBuf,
    _file: std::fs::File,
}

impl Drop for RunnerProcessLock {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            warn!(
                lock_path = %self.path.display(),
                error = %err,
                "BOT_RUNNER_LOCK_RELEASE_FAILED"
            );
        }
    }
}

fn parse_lock_pid(content: &str) -> Option<u32> {
    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "pid" {
            continue;
        }
        if let Ok(pid) = value.trim().parse::<u32>() {
            return Some(pid);
        }
    }
    None
}

fn is_lock_stale(path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return true;
    };
    let Some(pid) = parse_lock_pid(&content) else {
        return true;
    };
    let proc_dir = PathBuf::from(format!("/proc/{pid}"));
    if !proc_dir.exists() {
        return true;
    }
    let Ok(cmdline_raw) = fs::read(proc_dir.join("cmdline")) else {
        // If process metadata is inaccessible, assume lock owner may still be alive.
        return false;
    };
    let cmdline = String::from_utf8_lossy(&cmdline_raw);
    if cmdline.is_empty() {
        return false;
    }
    !cmdline.contains("bot-runner")
}

fn acquire_runner_process_lock() -> Result<RunnerProcessLock> {
    let lock_path = env::var("BOT_RUNNER_LOCK_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(BOT_RUNNER_LOCK_PATH_DEFAULT));

    for _ in 0..2 {
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                let _ = writeln!(file, "pid={}", std::process::id());
                return Ok(RunnerProcessLock {
                    path: lock_path,
                    _file: file,
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                if is_lock_stale(&lock_path) {
                    let _ = fs::remove_file(&lock_path);
                    continue;
                }
                anyhow::bail!(
                    "another bot-runner process appears active (lock: {})",
                    lock_path.display()
                );
            }
            Err(err) => {
                return Err(err.into());
            }
        }
    }

    anyhow::bail!(
        "failed to acquire bot-runner lock after stale cleanup (lock: {})",
        lock_path.display()
    )
}

fn crossed_above_strict(
    previous_price: Option<f64>,
    current_price: f64,
    trigger_price: f64,
) -> bool {
    previous_price
        .map(|prev| prev < trigger_price && current_price >= trigger_price)
        .unwrap_or(false)
}

fn crossed_below_strict(
    previous_price: Option<f64>,
    current_price: f64,
    trigger_price: f64,
) -> bool {
    previous_price
        .map(|prev| prev > trigger_price && current_price <= trigger_price)
        .unwrap_or(false)
}

fn evaluate_trigger_market_price_condition(
    previous_price: Option<f64>,
    current_price: f64,
    trigger_price: f64,
    trigger_condition: &str,
    allow_first_tick_threshold: bool,
    max_price: Option<f64>,
) -> (bool, &'static str) {
    match trigger_condition {
        "level_above" => {
            let in_range = current_price >= trigger_price
                && max_price.map_or(true, |mp| current_price <= mp);
            if in_range {
                if let Some(mp) = max_price {
                    if current_price > mp {
                        return (false, "above_max_price");
                    }
                    return (true, "level_in_range");
                }
                return (true, "level_threshold_met");
            }
            if max_price.is_some() && current_price > max_price.unwrap_or(f64::INFINITY) {
                return (false, "above_max_price");
            }
            if previous_price.is_none() && !allow_first_tick_threshold {
                (false, "no_previous")
            } else {
                (false, "level_not_met")
            }
        }
        "level_below" => {
            let in_range = current_price <= trigger_price
                && max_price.map_or(true, |mp| current_price <= mp);
            if in_range {
                return (true, "level_threshold_met");
            }
            if max_price.is_some() && current_price > max_price.unwrap_or(f64::INFINITY) {
                return (false, "above_max_price");
            }
            if previous_price.is_none() && !allow_first_tick_threshold {
                (false, "no_previous")
            } else {
                (false, "level_not_met")
            }
        }
        "cross_above" => {
            let crossed = if let Some(mp) = max_price {
                if current_price > mp {
                    let crossed_upper_bound = previous_price
                        .map(|prev| prev < trigger_price)
                        .unwrap_or(allow_first_tick_threshold);
                    if crossed_upper_bound {
                        return (false, "above_max_price");
                    }
                }
                let in_range = current_price >= trigger_price && current_price <= mp;
                if let Some(prev) = previous_price {
                    let prev_in_range = prev >= trigger_price && prev <= mp;
                    if !prev_in_range && in_range {
                        if prev < trigger_price {
                            Some("range_entry_from_below")
                        } else if prev > mp {
                            Some("range_entry_from_above")
                        } else {
                            Some("cross_detected")
                        }
                    } else {
                        None
                    }
                } else if allow_first_tick_threshold && in_range {
                    Some("first_tick_in_range")
                } else {
                    None
                }
            } else if let Some(prev) = previous_price {
                if prev < trigger_price && current_price >= trigger_price {
                    Some("cross_detected")
                } else {
                    None
                }
            } else if allow_first_tick_threshold && current_price >= trigger_price {
                Some("first_tick_threshold")
            } else {
                None
            };
            match crossed {
                Some(mode) => {
                    if let Some(mp) = max_price {
                        if current_price > mp {
                            return (false, "above_max_price");
                        }
                    }
                    (true, mode)
                }
                None => {
                    if previous_price.is_none() && !allow_first_tick_threshold {
                        (false, "no_previous")
                    } else {
                        (false, "no_cross")
                    }
                }
            }
        }
        "cross_below" => {
            let crossed = if let Some(prev) = previous_price {
                if prev > trigger_price && current_price <= trigger_price {
                    Some("cross_detected")
                } else {
                    None
                }
            } else if allow_first_tick_threshold && current_price <= trigger_price {
                Some("first_tick_threshold")
            } else {
                None
            };
            match crossed {
                Some(mode) => {
                    if let Some(mp) = max_price {
                        if current_price > mp {
                            return (false, "above_max_price");
                        }
                    }
                    (true, mode)
                }
                None => {
                    if previous_price.is_none() && !allow_first_tick_threshold {
                        (false, "no_previous")
                    } else {
                        (false, "no_cross")
                    }
                }
            }
        }
        _ => (false, "unsupported_condition"),
    }
}

fn is_supported_market_price_trigger_condition(trigger_condition: &str) -> bool {
    matches!(
        trigger_condition,
        "cross_above" | "cross_below" | "level_above" | "level_below"
    )
}

fn market_price_trigger_condition_requires_once(trigger_condition: &str) -> bool {
    matches!(trigger_condition, "level_above" | "level_below")
}

fn should_apply_ws_cross_confirmed_short_circuit(
    ws_sourced: bool,
    ws_evaluation_mode_from_step: &str,
    ws_hard_ignore_reason: Option<&str>,
) -> bool {
    ws_sourced
        && ws_evaluation_mode_from_step == "cross_confirmed"
        && ws_hard_ignore_reason.is_none()
}

fn should_allow_ws_first_tick_threshold_override(
    ws_sourced: bool,
    node_type: &str,
    allow_first_tick_replay: bool,
    ws_evaluation_mode_from_step: &str,
    ws_hard_ignore_reason: Option<&str>,
) -> bool {
    ws_sourced
        && node_type == "trigger.market_price"
        && allow_first_tick_replay
        && matches!(
            ws_evaluation_mode_from_step,
            "first_tick_threshold" | "first_tick_in_range"
        )
        && ws_hard_ignore_reason.is_none()
}

fn is_ws_cross_confirmed_unexpected_fail(
    ws_sourced: bool,
    ws_evaluation_mode_from_step: &str,
    pass: bool,
    ws_hard_ignore_reason: Option<&str>,
) -> bool {
    ws_sourced
        && ws_evaluation_mode_from_step == "cross_confirmed"
        && !pass
        && ws_hard_ignore_reason.is_none()
}

fn market_price_confirmation_ms(node_spec: &WsOpenPositionPriceNodeSpec) -> Option<i64> {
    if node_spec.node_type != "trigger.market_price" {
        return None;
    }
    node_spec.confirmation_ms.filter(|value| *value > 0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum WsPriceMode {
    Composite,
    Midpoint,
    Raw,
    LastTrade,
    SiteDisplay,
    BestBid,
    BestAsk,
}

impl WsPriceMode {
    fn parse(raw: Option<&str>, default: Self) -> Self {
        let normalized = raw.map(str::trim).unwrap_or_default().to_ascii_lowercase();
        match normalized.as_str() {
            "composite" => Self::Composite,
            "midpoint" | "orderbook_midpoint" | "mid" => Self::Midpoint,
            "raw" | "trade" => Self::Raw,
            "last_trade" | "last_trade_price" => Self::LastTrade,
            "site_display" | "display" => Self::SiteDisplay,
            "best_bid" | "bid" => Self::BestBid,
            "best_ask" | "ask" => Self::BestAsk,
            _ => default,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Composite => "composite",
            Self::Midpoint => "midpoint",
            Self::Raw => "raw",
            Self::LastTrade => "last_trade",
            Self::SiteDisplay => "site_display",
            Self::BestBid => "best_bid",
            Self::BestAsk => "best_ask",
        }
    }
}
