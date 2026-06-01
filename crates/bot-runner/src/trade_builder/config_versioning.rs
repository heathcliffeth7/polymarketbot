
pub fn generate_config_version() -> String {
    format!("v{}", Utc::now().timestamp())
}

pub fn diff_json(old: &Value, new: &Value) -> Value {
    match (old, new) {
        (Value::Object(old_map), Value::Object(new_map)) => {
            let mut diff = serde_json::Map::new();

            for (key, old_val) in old_map {
                if let Some(new_val) = new_map.get(key) {
                    if old_val != new_val {
                        diff.insert(
                            key.clone(),
                            json!({
                                "old": old_val,
                                "new": new_val
                            }),
                        );
                    }
                } else {
                    diff.insert(
                        key.clone(),
                        json!({
                            "old": old_val,
                            "new": null,
                            "change": "removed"
                        }),
                    );
                }
            }

            for (key, new_val) in new_map {
                if !old_map.contains_key(key) {
                    diff.insert(
                        key.clone(),
                        json!({
                            "old": null,
                            "new": new_val,
                            "change": "added"
                        }),
                    );
                }
            }

            Value::Object(diff)
        }
        (old_val, new_val) => {
            if old_val != new_val {
                json!({
                    "old": old_val,
                    "new": new_val
                })
            } else {
                Value::Object(serde_json::Map::new())
            }
        }
    }
}

pub async fn record_config_change(
    repo: &bot_infra::db::PostgresRepository,
    changed_by: String,
    change_reason: String,
    old_config: Value,
    new_config: Value,
) -> anyhow::Result<String> {
    let version = generate_config_version();
    let changed_fields = diff_json(&old_config, &new_config);

    repo.insert_config_change_log(&bot_infra::db::ConfigChangeLogInput {
        config_version: version.clone(),
        changed_by: Some(changed_by),
        change_reason: Some(change_reason),
        changed_fields,
        full_config_snapshot: new_config,
    })
    .await?;

    Ok(version)
}

pub async fn get_active_config_version(
    repo: &bot_infra::db::PostgresRepository,
) -> Option<String> {
    repo.get_active_config_version().await.ok().flatten()
}

#[cfg(test)]
mod config_versioning_tests {
    use super::*;

    #[test]
    fn diff_json_detects_value_change() {
        let old = json!({"maxPrice": 0.87, "slEnabled": true});
        let new = json!({"maxPrice": 0.90, "slEnabled": true});
        let diff = diff_json(&old, &new);

        assert!(diff.get("maxPrice").is_some());
        assert!(diff.get("slEnabled").is_none());
        assert_eq!(diff["maxPrice"]["old"], 0.87);
        assert_eq!(diff["maxPrice"]["new"], 0.90);
    }

    #[test]
    fn diff_json_detects_added_key() {
        let old = json!({"maxPrice": 0.87});
        let new = json!({"maxPrice": 0.87, "slEnabled": false});
        let diff = diff_json(&old, &new);

        assert_eq!(diff["slEnabled"]["change"], "added");
        assert_eq!(diff["slEnabled"]["new"], false);
    }

    #[test]
    fn diff_json_detects_removed_key() {
        let old = json!({"maxPrice": 0.87, "slEnabled": true});
        let new = json!({"maxPrice": 0.87});
        let diff = diff_json(&old, &new);

        assert_eq!(diff["slEnabled"]["change"], "removed");
    }

    #[test]
    fn diff_json_empty_when_same() {
        let val = json!({"maxPrice": 0.87});
        let diff = diff_json(&val, &val);

        assert!(diff.as_object().unwrap().is_empty());
    }

    #[test]
    fn generate_config_version_starts_with_v() {
        let version = generate_config_version();
        assert!(version.starts_with("v"));
    }
}
