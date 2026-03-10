use super::*;

pub(crate) fn decrypt_config_string_if_needed(field_name: &str, raw_value: &str) -> Result<String> {
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if !trimmed.starts_with(CONFIG_ENC_PREFIX) {
        return Ok(trimmed.to_string());
    }

    let payload = &trimmed[CONFIG_ENC_PREFIX.len()..];
    let decoded = BASE64_STANDARD
        .decode(payload)
        .with_context(|| format!("decoding encrypted config value for {field_name}"))?;
    anyhow::ensure!(
        decoded.len() > CONFIG_ENC_NONCE_LEN + CONFIG_ENC_TAG_LEN,
        "encrypted config value too short for {field_name}"
    );

    let key_material = load_config_encryption_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key_material)
        .map_err(|_| anyhow::anyhow!("invalid config encryption key length"))?;

    let nonce = Nonce::from_slice(&decoded[..CONFIG_ENC_NONCE_LEN]);
    let ciphertext = &decoded[CONFIG_ENC_NONCE_LEN..];
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow::anyhow!("decrypting encrypted config value for {field_name}"))?;
    String::from_utf8(plaintext)
        .with_context(|| format!("encrypted config value is not valid utf-8 for {field_name}"))
        .map(|value| value.trim().to_string())
}

pub(crate) fn load_config_encryption_key() -> Result<Vec<u8>> {
    let encoded = env::var("CONFIG_ENCRYPTION_KEY")
        .context("CONFIG_ENCRYPTION_KEY is required to decrypt stored config values")?;
    let trimmed = encoded.trim();
    anyhow::ensure!(
        !trimmed.is_empty(),
        "CONFIG_ENCRYPTION_KEY is required to decrypt stored config values"
    );
    let decoded = BASE64_STANDARD
        .decode(trimmed)
        .context("CONFIG_ENCRYPTION_KEY must be valid base64")?;
    anyhow::ensure!(
        decoded.len() == 32,
        "CONFIG_ENCRYPTION_KEY must decode to exactly 32 bytes"
    );
    Ok(decoded)
}
