use super::*;

pub(crate) fn load_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str::<T>(&raw).with_context(|| format!("parsing {}", path.display()))
}

pub(crate) fn load_toml_or_default<T: for<'de> Deserialize<'de> + Default>(
    path: &Path,
) -> Result<T> {
    if !path.exists() {
        return Ok(T::default());
    }
    load_toml(path)
}

fn load_toml_as_json_value(path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let toml_value: toml::Value =
        toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    serde_json::to_value(toml_value)
        .with_context(|| format!("converting TOML to JSON for {}", path.display()))
}

pub(crate) fn load_json_or_toml<T: for<'de> Deserialize<'de>>(
    payload: Option<&Value>,
    path: &Path,
) -> Result<T> {
    if let Some(value) = payload {
        return serde_json::from_value(value.clone())
            .with_context(|| format!("parsing stored config payload for {}", path.display()));
    }
    load_toml(path)
}

pub(crate) fn load_json_or_default<T: for<'de> Deserialize<'de> + Default>(
    payload: Option<&Value>,
) -> Result<T> {
    if let Some(value) = payload {
        return serde_json::from_value(value.clone()).context("parsing stored config payload");
    }
    Ok(T::default())
}

pub(crate) fn load_json_or_toml_or_default<T: for<'de> Deserialize<'de> + Default>(
    payload: Option<&Value>,
    path: &Path,
) -> Result<T> {
    if let Some(value) = payload {
        return serde_json::from_value(value.clone())
            .with_context(|| format!("parsing stored config payload for {}", path.display()));
    }
    if path.exists() {
        return load_toml(path);
    }
    Ok(T::default())
}

/// Load config by merging DB-stored JSON with TOML file fallback.
/// TOML provides the base values; DB non-empty/truthy values override.
/// Empty strings, `false`, and null in DB are treated as "not set" and
/// fall back to the TOML value. This handles seeded configs where
/// sensitive fields are intentionally cleared.
pub(crate) fn load_json_merged_with_toml<T>(
    payload: Option<&Value>,
    path: &Path,
) -> Result<T>
where
    T: serde::de::DeserializeOwned + Default,
{
    let toml_value = if path.exists() {
        load_toml_as_json_value(path).ok()
    } else {
        None
    };

    let merged = match (payload.cloned(), toml_value) {
        (Some(db), Some(toml)) => merge_json_values(toml, db),
        (Some(db), None) => db,
        (None, Some(toml)) => toml,
        (None, None) => return Ok(T::default()),
    };

    serde_json::from_value(merged)
        .with_context(|| format!("parsing merged config for {}", path.display()))
}

/// Merge two JSON objects: `base` (TOML) overlaid with non-empty `overlay` (DB) values.
/// Rules per field:
/// - DB value is string and non-empty → use DB
/// - DB value is bool `true` → use DB
/// - DB value is number → use DB
/// - DB value is bool `false`, empty string, or null → use base (TOML)
/// - Both are objects → recurse
fn merge_json_values(mut base: Value, overlay: Value) -> Value {
    let Some(base_map) = base.as_object_mut() else {
        return overlay;
    };
    let Some(overlay_map) = overlay.as_object() else {
        return overlay;
    };

    for (key, overlay_val) in overlay_map {
        let base_val = base_map.get(key);
        let merged_val = match (base_val.cloned(), overlay_val) {
            (Some(Value::Object(base_obj)), Value::Object(overlay_obj)) => {
                merge_json_values(Value::Object(base_obj), Value::Object(overlay_obj.clone()))
            }
            (Some(_), Value::Null) => continue, // keep base
            (Some(_), Value::String(s)) if s.is_empty() => continue, // keep base
            (Some(_), Value::Bool(false)) => continue, // keep base for seeded defaults
            _ => overlay_val.clone(),
        };
        base_map.insert(key.clone(), merged_val);
    }

    base
}
