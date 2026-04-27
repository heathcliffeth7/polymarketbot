use anyhow::{Context, Result};
use base64::engine::general_purpose::URL_SAFE;
use base64::Engine as _;
use ethers::{
    abi::{encode, Token},
    signers::LocalWallet,
    types::{Address, H256, U256},
    utils::keccak256,
};
use hmac::{Hmac, Mac as _};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, env, sync::LazyLock};
use tracing::debug;

pub const POLY_ADDRESS: &str = "POLY_ADDRESS";
pub const POLY_SIGNATURE: &str = "POLY_SIGNATURE";
pub const POLY_TIMESTAMP: &str = "POLY_TIMESTAMP";
pub const POLY_API_KEY: &str = "POLY_API_KEY";
pub const POLY_PASSPHRASE: &str = "POLY_PASSPHRASE";

#[derive(Debug, Clone)]
pub struct ApiCredentials {
    pub address: String,
    pub key: String,
    pub secret: String,
    pub passphrase: String,
}

impl ApiCredentials {
    pub fn from_env(
        address_var: &str,
        key_var: &str,
        secret_var: &str,
        passphrase_var: &str,
    ) -> Result<Self> {
        let address =
            env::var(address_var).with_context(|| format!("missing env: {address_var}"))?;
        let key = env::var(key_var).with_context(|| format!("missing env: {key_var}"))?;
        let secret = env::var(secret_var).with_context(|| format!("missing env: {secret_var}"))?;
        let passphrase =
            env::var(passphrase_var).with_context(|| format!("missing env: {passphrase_var}"))?;
        Ok(Self {
            address,
            key,
            secret,
            passphrase,
        })
    }
}

pub trait HeaderSigner: Send + Sync {
    fn signed_headers(
        &self,
        timestamp: i64,
        method: &str,
        request_path: &str,
        body: Option<&str>,
    ) -> Result<HashMap<String, String>>;
}

#[derive(Debug, Clone)]
pub struct ClobHeaderSigner {
    pub creds: ApiCredentials,
}

impl HeaderSigner for ClobHeaderSigner {
    fn signed_headers(
        &self,
        timestamp: i64,
        method: &str,
        request_path: &str,
        body: Option<&str>,
    ) -> Result<HashMap<String, String>> {
        // py-clob-client parity:
        // message = f"{timestamp}{method}{request_path}{body.replace(\"'\", '\"')}"
        // signature = base64.urlsafe_b64encode(HMAC_SHA256(base64.urlsafe_b64decode(secret), message))
        // NOTE: Polymarket CLOB API expects only the path (no query string) in the HMAC message.
        let sign_path = request_path.split('?').next().unwrap_or(request_path);
        let decoded_secret = decode_clob_api_secret(&self.creds.secret)?;
        let mut mac = Hmac::<Sha256>::new_from_slice(&decoded_secret)?;
        let mut message = format!("{timestamp}{method}{sign_path}");
        if let Some(raw) = body {
            message.push_str(&raw.replace('\'', "\""));
        }
        mac.update(message.as_bytes());
        let signature = URL_SAFE.encode(mac.finalize().into_bytes());
        let body_hash_prefix = body.map(|raw| {
            let digest = Sha256::digest(raw.as_bytes());
            digest
                .iter()
                .take(6)
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        });
        let signature_prefix: String = signature.chars().take(12).collect();

        let mut headers = HashMap::new();
        headers.insert(POLY_ADDRESS.to_string(), self.creds.address.clone());
        headers.insert(POLY_SIGNATURE.to_string(), signature);
        headers.insert(POLY_TIMESTAMP.to_string(), timestamp.to_string());
        headers.insert(POLY_API_KEY.to_string(), self.creds.key.clone());
        headers.insert(POLY_PASSPHRASE.to_string(), self.creds.passphrase.clone());
        debug!(
            method,
            request_path,
            timestamp,
            has_body = body.is_some(),
            body_len = body.map(|raw| raw.len()).unwrap_or(0),
            body_hash_prefix = ?body_hash_prefix,
            api_address = %self.creds.address,
            api_key_prefix = %masked_prefix(&self.creds.key, 8),
            signature_prefix = %signature_prefix,
            secret_len = self.creds.secret.trim().len(),
            passphrase_len = self.creds.passphrase.trim().len(),
            "CLOB_SIGNED_HEADERS_BUILT"
        );
        Ok(headers)
    }
}

fn decode_clob_api_secret(raw: &str) -> Result<Vec<u8>> {
    let trimmed = raw.trim();
    anyhow::ensure!(!trimmed.is_empty(), "POLY API secret is empty");

    let remainder = trimmed.len() % 4;
    anyhow::ensure!(
        remainder != 1,
        "POLY API secret has invalid base64url length"
    );

    let normalized = if remainder == 0 {
        trimmed.to_string()
    } else {
        format!("{trimmed}{}", "=".repeat(4 - remainder))
    };

    URL_SAFE
        .decode(normalized.as_bytes())
        .context("decode POLY API secret as base64url")
}

fn masked_prefix(raw: &str, take: usize) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "<empty>".to_string();
    }
    let prefix: String = trimmed.chars().take(take).collect();
    format!("{prefix}***")
}

pub fn unix_now_secs() -> Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?;
    Ok(now.as_secs() as i64)
}

pub fn unix_now_millis() -> Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?;
    Ok(now.as_millis() as i64)
}

const ORDER_TYPE_STR: &str = "Order(uint256 salt,address maker,address signer,\
     uint256 tokenId,uint256 makerAmount,uint256 takerAmount,\
     uint8 side,uint8 signatureType,uint256 timestamp,bytes32 metadata,bytes32 builder)";

const DOMAIN_TYPE_STR: &str =
    "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";

static ORDER_TYPE_HASH: LazyLock<[u8; 32]> = LazyLock::new(|| keccak256(ORDER_TYPE_STR));
static DOMAIN_TYPE_HASH: LazyLock<[u8; 32]> = LazyLock::new(|| keccak256(DOMAIN_TYPE_STR));
static DOMAIN_NAME_HASH: LazyLock<[u8; 32]> =
    LazyLock::new(|| keccak256("Polymarket CTF Exchange"));
static DOMAIN_VERSION_HASH: LazyLock<[u8; 32]> = LazyLock::new(|| keccak256("2"));

pub fn domain_separator_for_exchange(chain_id: u64, exchange_address: Address) -> [u8; 32] {
    keccak256(encode(&[
        Token::FixedBytes(DOMAIN_TYPE_HASH.to_vec()),
        Token::FixedBytes(DOMAIN_NAME_HASH.to_vec()),
        Token::FixedBytes(DOMAIN_VERSION_HASH.to_vec()),
        Token::Uint(U256::from(chain_id)),
        Token::Address(exchange_address),
    ]))
}

pub fn sign_order_eip712_with_domain_separator(
    wallet: &LocalWallet,
    domain_separator: [u8; 32],
    salt: U256,
    maker: Address,
    signer: Address,
    token_id: U256,
    maker_amount: U256,
    taker_amount: U256,
    side: u8,
    sig_type: u64,
    timestamp: U256,
    metadata: [u8; 32],
    builder: [u8; 32],
) -> Result<String> {
    let struct_hash: [u8; 32] = keccak256(encode(&[
        Token::FixedBytes(ORDER_TYPE_HASH.to_vec()),
        Token::Uint(salt),
        Token::Address(maker),
        Token::Address(signer),
        Token::Uint(token_id),
        Token::Uint(maker_amount),
        Token::Uint(taker_amount),
        Token::Uint(U256::from(side as u64)),
        Token::Uint(U256::from(sig_type)),
        Token::Uint(timestamp),
        Token::FixedBytes(metadata.to_vec()),
        Token::FixedBytes(builder.to_vec()),
    ]));

    let mut digest_input = [0u8; 66];
    digest_input[0] = 0x19;
    digest_input[1] = 0x01;
    digest_input[2..34].copy_from_slice(&domain_separator);
    digest_input[34..].copy_from_slice(&struct_hash);
    let final_hash = keccak256(digest_input);

    let signature = wallet
        .sign_hash(H256::from(final_hash))
        .map_err(|e| anyhow::anyhow!("EIP-712 sign_hash: {e}"))?;

    let sig_bytes: [u8; 65] = signature.into();
    let sig_hex = sig_bytes.iter().fold("0x".to_string(), |mut s: String, b| {
        use std::fmt::Write;
        write!(s, "{b:02x}").unwrap();
        s
    });

    Ok(sig_hex)
}

/// Signs an order using EIP-712 structured data signing.
/// Returns the hex-encoded signature string (0x-prefixed, 65 bytes).
///
/// For EOA orders: signer == maker, sig_type == 0
/// For Gnosis Safe orders: signer == EOA (the private key holder), maker == proxy, sig_type == 2
pub fn sign_order_eip712(
    wallet: &LocalWallet,
    chain_id: u64,
    exchange_address: Address,
    salt: U256,
    maker: Address,
    signer: Address,
    token_id: U256,
    maker_amount: U256,
    taker_amount: U256,
    side: u8,
    sig_type: u64,
    timestamp: U256,
    metadata: [u8; 32],
    builder: [u8; 32],
) -> Result<String> {
    sign_order_eip712_with_domain_separator(
        wallet,
        domain_separator_for_exchange(chain_id, exchange_address),
        salt,
        maker,
        signer,
        token_id,
        maker_amount,
        taker_amount,
        side,
        sig_type,
        timestamp,
        metadata,
        builder,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l2_hmac_matches_py_clob_client_shape() {
        let signer = ClobHeaderSigner {
            creds: ApiCredentials {
                address: "0xabc".to_string(),
                key: "k".to_string(),
                secret: "YWFhYQ==".to_string(),
                passphrase: "p".to_string(),
            },
        };

        let body = "{\"market\":\"btc-updown-5m-1\",\"side\":\"buy\"}";
        let headers = signer
            .signed_headers(1_700_000_000, "POST", "/order", Some(body))
            .expect("headers");

        assert_eq!(
            headers.get(POLY_SIGNATURE).map(String::as_str),
            Some("NktcyCPI12LWeG-3Xg_r2Hpq9fX-PuoNl0N7nCwXdk0=")
        );
        assert_eq!(
            headers.get(POLY_TIMESTAMP).map(String::as_str),
            Some("1700000000")
        );
    }

    #[test]
    fn l2_hmac_accepts_unpadded_base64url_secret() {
        let body = "{\"market\":\"btc-updown-5m-1\",\"side\":\"buy\"}";

        let padded = ClobHeaderSigner {
            creds: ApiCredentials {
                address: "0xabc".to_string(),
                key: "k".to_string(),
                secret: "YWFhYQ==".to_string(),
                passphrase: "p".to_string(),
            },
        };
        let unpadded = ClobHeaderSigner {
            creds: ApiCredentials {
                address: "0xabc".to_string(),
                key: "k".to_string(),
                secret: "YWFhYQ".to_string(),
                passphrase: "p".to_string(),
            },
        };

        let padded_headers = padded
            .signed_headers(1_700_000_000, "POST", "/order", Some(body))
            .expect("padded headers");
        let unpadded_headers = unpadded
            .signed_headers(1_700_000_000, "POST", "/order", Some(body))
            .expect("unpadded headers");

        assert_eq!(
            padded_headers.get(POLY_SIGNATURE),
            unpadded_headers.get(POLY_SIGNATURE)
        );
        assert_eq!(
            padded_headers.get(POLY_TIMESTAMP),
            unpadded_headers.get(POLY_TIMESTAMP)
        );
    }

    #[test]
    fn l2_hmac_rejects_invalid_base64url_secret() {
        let signer = ClobHeaderSigner {
            creds: ApiCredentials {
                address: "0xabc".to_string(),
                key: "k".to_string(),
                secret: "%%%".to_string(),
                passphrase: "p".to_string(),
            },
        };

        let err = signer
            .signed_headers(1_700_000_000, "GET", "/data/trades", None)
            .expect_err("invalid secret should fail");

        assert!(err
            .to_string()
            .contains("decode POLY API secret as base64url"));
    }

    #[test]
    fn cached_domain_separator_signing_matches_v2_signing() {
        let wallet = "0000000000000000000000000000000000000000000000000000000000000001"
            .parse::<LocalWallet>()
            .unwrap();
        let exchange_address = Address::from_low_u64_be(1);
        let salt = U256::from(7u64);
        let maker = Address::from_low_u64_be(2);
        let signer = Address::from_low_u64_be(3);
        let token_id = U256::from(4u64);
        let maker_amount = U256::from(5u64);
        let taker_amount = U256::from(6u64);
        let timestamp = U256::from(1_713_398_400_000u64);
        let metadata = [0u8; 32];
        let builder = [0u8; 32];

        let direct = sign_order_eip712(
            &wallet,
            137,
            exchange_address,
            salt,
            maker,
            signer,
            token_id,
            maker_amount,
            taker_amount,
            0,
            2,
            timestamp,
            metadata,
            builder,
        )
        .unwrap();
        let cached = sign_order_eip712_with_domain_separator(
            &wallet,
            domain_separator_for_exchange(137, exchange_address),
            salt,
            maker,
            signer,
            token_id,
            maker_amount,
            taker_amount,
            0,
            2,
            timestamp,
            metadata,
            builder,
        )
        .unwrap();

        assert_eq!(cached, direct);
    }
}
