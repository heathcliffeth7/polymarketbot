#![cfg(test)]

use super::defaults::{CONFIG_ENC_NONCE_LEN, CONFIG_ENC_PREFIX};
use super::models::ClaimConfig;
use super::validate::validate_claim_private_key;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use std::env;
use std::sync::{Mutex, OnceLock};

fn test_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct ScopedConfigEncryptionKey {
    previous: Option<String>,
}

impl ScopedConfigEncryptionKey {
    fn set(encoded_key: &str) -> Self {
        let previous = env::var("CONFIG_ENCRYPTION_KEY").ok();
        env::set_var("CONFIG_ENCRYPTION_KEY", encoded_key);
        Self { previous }
    }
}

impl Drop for ScopedConfigEncryptionKey {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_deref() {
            env::set_var("CONFIG_ENCRYPTION_KEY", previous);
        } else {
            env::remove_var("CONFIG_ENCRYPTION_KEY");
        }
    }
}

fn encrypt_config_string_for_test(raw: &str) -> Result<(String, String)> {
    let key_material = [17_u8; 32];
    let encoded_key = BASE64_STANDARD.encode(key_material);
    let cipher = Aes256Gcm::new_from_slice(&key_material)?;
    let nonce_bytes = [9_u8; CONFIG_ENC_NONCE_LEN];
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, raw.as_bytes())
        .map_err(|_| anyhow::anyhow!("test encryption failed"))?;
    let encrypted = format!(
        "{CONFIG_ENC_PREFIX}{}",
        BASE64_STANDARD.encode([nonce_bytes.as_slice(), ciphertext.as_slice()].concat())
    );
    Ok((encoded_key, encrypted))
}

#[test]
fn claim_private_key_validation_ignores_disabled_encrypted_value() {
    let claim = ClaimConfig {
        enabled: false,
        private_key: "enc:v1:not-a-real-key".to_string(),
        ..ClaimConfig::default()
    };

    assert!(validate_claim_private_key(&claim).is_ok());
}

#[test]
fn claim_private_key_validation_accepts_enabled_encrypted_value() {
    let _guard = test_env_lock().lock().unwrap();
    let (encoded_key, encrypted_key) = encrypt_config_string_for_test(
        "0x1111111111111111111111111111111111111111111111111111111111111111",
    )
    .unwrap();
    let _env_guard = ScopedConfigEncryptionKey::set(&encoded_key);
    let claim = ClaimConfig {
        enabled: true,
        private_key: encrypted_key,
        ..ClaimConfig::default()
    };

    assert!(validate_claim_private_key(&claim).is_ok());
}

#[test]
fn claim_private_key_validation_rejects_invalid_decrypted_value() {
    let _guard = test_env_lock().lock().unwrap();
    let (encoded_key, encrypted_key) = encrypt_config_string_for_test("not-a-private-key").unwrap();
    let _env_guard = ScopedConfigEncryptionKey::set(&encoded_key);
    let claim = ClaimConfig {
        enabled: true,
        private_key: encrypted_key,
        ..ClaimConfig::default()
    };

    let err = validate_claim_private_key(&claim).unwrap_err();
    assert!(err
        .to_string()
        .contains("claim.private_key must be a valid 0x private key when provided"));
}
