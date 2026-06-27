const ML_ENTRY_QUALITY_SHADOW_ENABLED_ENV: &str = "ML_ENTRY_QUALITY_SHADOW_ENABLED";
const ML_ENTRY_QUALITY_PYTHON_BIN_ENV: &str = "ML_ENTRY_QUALITY_PYTHON_BIN";
const ML_ENTRY_QUALITY_MODEL_DIR_ENV: &str = "ML_ENTRY_QUALITY_MODEL_DIR";
const ML_ENTRY_QUALITY_SCORER_ENV: &str = "ML_ENTRY_QUALITY_SCORER";
const ML_ENTRY_QUALITY_TIMEOUT_MS_ENV: &str = "ML_ENTRY_QUALITY_TIMEOUT_MS";
const ML_ENTRY_QUALITY_DEFAULT_PYTHON_BIN: &str = "python3";
const ML_ENTRY_QUALITY_DEFAULT_MODEL_DIR: &str = "artifacts/ml_entry_quality/v1";
const ML_ENTRY_QUALITY_DEFAULT_SCORER: &str = "scripts/ml_entry_quality/score_payload.py";
const ML_ENTRY_QUALITY_DEFAULT_TIMEOUT_MS: u64 = 250;

fn ml_entry_quality_env_bool(name: &str) -> Option<bool> {
    let raw = std::env::var(name).ok()?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn ml_entry_quality_config_enabled(payload: &Value) -> Option<bool> {
    [
        &["node_snapshot", "resolved_order_input", "mlEntryQualityShadowEnabled"][..],
        &[
            "node_snapshot",
            "action_node",
            "config",
            "mlEntryQualityShadowEnabled",
        ][..],
    ]
    .iter()
    .find_map(|path| {
        let mut current = payload;
        for key in *path {
            current = current.get(*key)?;
        }
        current.as_bool()
    })
}

fn ml_entry_quality_shadow_enabled_from(payload: &Value, env_enabled: Option<bool>) -> bool {
    env_enabled
        .or_else(|| ml_entry_quality_config_enabled(payload))
        .unwrap_or(false)
}

fn ml_entry_quality_timeout_ms() -> u64 {
    std::env::var(ML_ENTRY_QUALITY_TIMEOUT_MS_ENV)
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(ML_ENTRY_QUALITY_DEFAULT_TIMEOUT_MS)
}

fn ml_entry_quality_python_bin() -> String {
    std::env::var(ML_ENTRY_QUALITY_PYTHON_BIN_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| ML_ENTRY_QUALITY_DEFAULT_PYTHON_BIN.to_string())
}

fn ml_entry_quality_model_dir() -> String {
    std::env::var(ML_ENTRY_QUALITY_MODEL_DIR_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| ML_ENTRY_QUALITY_DEFAULT_MODEL_DIR.to_string())
}

fn ml_entry_quality_scorer_path() -> String {
    std::env::var(ML_ENTRY_QUALITY_SCORER_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| ML_ENTRY_QUALITY_DEFAULT_SCORER.to_string())
}

fn ml_entry_quality_resolve_path(raw: &str) -> std::path::PathBuf {
    let path = std::path::PathBuf::from(raw);
    if path.is_absolute() || path.exists() {
        return path;
    }
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .map(std::path::Path::to_path_buf)
        .unwrap_or(manifest_dir);
    repo_root.join(raw)
}

fn ml_entry_quality_merge_score(mut payload: Value, score: &Value) -> Value {
    let Some(payload_obj) = payload.as_object_mut() else {
        return payload;
    };
    for key in [
        "ml_entry_quality_score",
        "ml_stop_loss_risk",
        "ml_model_version",
        "ml_features_version",
        "ml_shadow_policy_version",
        "ml_shadow_decision",
    ] {
        if let Some(value) = score.get(key) {
            payload_obj.insert(key.to_string(), value.clone());
        }
    }
    payload
}

fn ml_entry_quality_score_payload_blocking_with_paths(
    payload_raw: String,
    python_bin: &str,
    scorer: std::path::PathBuf,
    model_dir: std::path::PathBuf,
    timeout_ms: u64,
) -> Option<Value> {
    if !scorer.exists() || !model_dir.exists() {
        return None;
    }

    let mut child = std::process::Command::new(python_bin)
        .arg(&scorer)
        .arg("--model-dir")
        .arg(&model_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;

    if let Some(mut stdin) = child.stdin.take() {
        if std::io::Write::write_all(&mut stdin, payload_raw.as_bytes()).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
    }

    let timeout = std::time::Duration::from_millis(timeout_ms);
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = child.wait_with_output().ok()?;
                if !output.status.success() {
                    debug!(
                        stderr = %String::from_utf8_lossy(&output.stderr),
                        "ML_ENTRY_QUALITY_SHADOW_SCORE_FAILED"
                    );
                    return None;
                }
                return serde_json::from_slice::<Value>(&output.stdout).ok();
            }
            Ok(None) if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                debug!("ML_ENTRY_QUALITY_SHADOW_SCORE_TIMEOUT");
                return None;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(5)),
            Err(err) => {
                debug!(error = %err, "ML_ENTRY_QUALITY_SHADOW_SCORE_WAIT_FAILED");
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn ml_entry_quality_score_payload_blocking(payload_raw: String) -> Option<Value> {
    ml_entry_quality_score_payload_blocking_with_paths(
        payload_raw,
        &ml_entry_quality_python_bin(),
        ml_entry_quality_resolve_path(&ml_entry_quality_scorer_path()),
        ml_entry_quality_resolve_path(&ml_entry_quality_model_dir()),
        ml_entry_quality_timeout_ms(),
    )
}

async fn maybe_score_ml_entry_quality_shadow(payload: Value) -> Value {
    if !ml_entry_quality_shadow_enabled_from(&payload, ml_entry_quality_env_bool(ML_ENTRY_QUALITY_SHADOW_ENABLED_ENV)) {
        return payload;
    }
    let Ok(payload_raw) = serde_json::to_string(&payload) else {
        return payload;
    };
    match tokio::task::spawn_blocking(move || ml_entry_quality_score_payload_blocking(payload_raw)).await {
        Ok(Some(score)) => ml_entry_quality_merge_score(payload, &score),
        Ok(None) => payload,
        Err(err) => {
            debug!(error = %err, "ML_ENTRY_QUALITY_SHADOW_JOIN_FAILED");
            payload
        }
    }
}

#[cfg(test)]
mod trade_builder_ml_entry_quality_shadow_tests {
    use super::*;

    #[test]
    fn shadow_is_disabled_by_default_without_config() {
        assert!(!ml_entry_quality_shadow_enabled_from(&json!({}), None));
    }

    #[test]
    fn shadow_can_be_enabled_from_node_snapshot_config() {
        let payload = json!({
            "node_snapshot": {
                "action_node": {
                    "config": {
                        "mlEntryQualityShadowEnabled": true
                    }
                }
            }
        });

        assert!(ml_entry_quality_shadow_enabled_from(&payload, None));
    }

    #[test]
    fn shadow_env_override_takes_precedence_over_config() {
        let payload = json!({
            "node_snapshot": {
                "action_node": {
                    "config": {
                        "mlEntryQualityShadowEnabled": true
                    }
                }
            }
        });

        assert!(!ml_entry_quality_shadow_enabled_from(&payload, Some(false)));
    }

    #[test]
    fn merge_adds_ml_fields_without_changing_decision() {
        let payload = json!({
            "decision": "allow",
            "decision_reason": "ptb_passed"
        });
        let merged = ml_entry_quality_merge_score(
            payload,
            &json!({
                "ml_entry_quality_score": 0.72,
                "ml_stop_loss_risk": 0.18,
                "ml_model_version": "ml_entry_quality_v1",
                "ml_features_version": "features_v1",
                "ml_shadow_policy_version": "static_balanced_v3",
                "ml_shadow_decision": "allow_like"
            }),
        );

        assert_eq!(merged["decision"], "allow");
        assert_eq!(merged["ml_entry_quality_score"], 0.72);
        assert_eq!(merged["ml_stop_loss_risk"], 0.18);
        assert_eq!(merged["ml_shadow_policy_version"], "static_balanced_v3");
        assert_eq!(merged["ml_shadow_decision"], "allow_like");
    }

    #[test]
    fn missing_artifact_fails_open_without_scoring() {
        let missing = std::path::PathBuf::from("/tmp/dextrabot-missing-ml-entry-quality-artifacts");
        let score = ml_entry_quality_score_payload_blocking_with_paths(
            "{}".to_string(),
            "python3",
            missing.join("score_payload.py"),
            missing.join("model"),
            1,
        );

        assert!(score.is_none());
    }
}
