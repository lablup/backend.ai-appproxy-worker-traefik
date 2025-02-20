use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::debug_log;

#[derive(Serialize, Deserialize, Debug)]
pub struct JwtHeader {
    alg: String,
    typ: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct JwtPayload {
    exp: Option<i64>, // Expiration time (Unix timestamp)
                      // Include other fields from your payload here
}

#[derive(Debug, Clone)]
pub struct JWTError {
    reason: String,
}

impl std::fmt::Display for JWTError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JWTError: {}", self.reason)
    }
}

impl Error for JWTError {}

macro_rules! unwrap_or_error {
    ($val:expr, $err: tt) => {
        match $val {
            Ok(i) => i,
            Err(e) => {
                return Err(JWTError {
                    reason: format!($err, e).to_string(),
                })
            }
        }
    };
}

pub fn validate_signature(
    alg: &str,
    header: &str,
    payload: &str,
    provided_signature: &str,
    secret: &[u8],
) -> Result<bool, JWTError> {
    // Check that the algorithm is HMAC-SHA256
    if alg != "HS256" {
        return Err(JWTError {
            reason: "Unsupported algorithm".to_string(),
        });
    }

    // Create the message to verify: header.payload
    let message = format!("{}.{}", header, payload);

    // Create HMAC-SHA256 instance
    let mut mac = unwrap_or_error!(
        Hmac::<Sha256>::new_from_slice(secret),
        "Failed to create HMAC secret: {:?}"
    );
    mac.update(message.as_bytes());

    // Get the computed HMAC
    let bytes = mac.finalize().into_bytes();
    let calculated_signature = bytes.as_slice();

    let provided_signature = unwrap_or_error!(
        BASE64_URL_SAFE_NO_PAD.decode(&provided_signature),
        "Failed to decode signature: {:?}"
    );

    if calculated_signature != provided_signature {
        return Err(JWTError {
            reason: "Invalid signature".to_string(),
        });
    }

    let payload = unwrap_or_error!(
        BASE64_URL_SAFE_NO_PAD.decode(payload),
        "Failed to decode payload: {:?}"
    );

    let parsed_payload: JwtPayload = unwrap_or_error!(
        serde_json::from_slice(&payload),
        "Failed to parse claims: {:?}"
    );

    // Validate expiration (if the `exp` claim exists)
    if let Some(expiration) = parsed_payload.exp {
        // Get current time as Unix timestamp (seconds since epoch)
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64;

        if current_time > expiration {
            Err(JWTError {
                reason: "JWT has expired".to_string(),
            })
        } else {
            Ok(true)
        }
    } else {
        Err(JWTError {
            reason: "JWT does not contain exp field".to_string(),
        })
    }
}

pub fn decode_jwt<T>(jwt: &str, secret: &str) -> Result<T, JWTError>
where
    T: for<'de> Deserialize<'de>, // T must be deserializable
{
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err(JWTError {
            reason: "Invalid JWT format".to_string(),
        });
    }

    // Decode header and payload (first and second parts of the JWT)
    debug_log!(
        "Header: {}, Payload: {}, Signature: {}",
        parts[0],
        parts[1],
        parts[2]
    );

    let header = unwrap_or_error!(
        BASE64_URL_SAFE_NO_PAD.decode(parts[0]),
        "Failed to decode header: {:?}"
    );
    let payload = unwrap_or_error!(
        BASE64_URL_SAFE_NO_PAD.decode(parts[1]),
        "Failed to decode payload: {:?}"
    );

    // Parse the header and payload into their respective types
    let parsed_header: JwtHeader = unwrap_or_error!(
        serde_json::from_slice(&header),
        "Failed to parse JWT header: {:?}"
    );

    let parsed_payload: T = unwrap_or_error!(
        serde_json::from_slice(&payload),
        "Failed to parse JWT claims: {:?}"
    );

    debug_log!("Using secret {}", secret);
    match validate_signature(
        parsed_header.alg.as_str(),
        &parts[0],
        &parts[1],
        &parts[2],
        secret.as_bytes(),
    ) {
        Ok(valid) => {
            if valid {
                Ok(parsed_payload)
            } else {
                Err(JWTError {
                    reason: "Invalid JWT string".to_string(),
                })
            }
        }
        Err(e) => Err(e),
    }
}
