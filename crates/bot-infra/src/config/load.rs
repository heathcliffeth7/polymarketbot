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
