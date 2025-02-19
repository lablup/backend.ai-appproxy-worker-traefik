use base64::{prelude::BASE64_URL_SAFE, Engine};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

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

pub fn validate_signature(
    alg: &str,
    header: &[u8],
    payload: &[u8],
    signature: &[u8],
    secret: &[u8],
) -> Result<bool, Box<dyn Error>> {
    // Check that the algorithm is HMAC-SHA256
    if alg != "HS256" {
        return Err("Unsupported algorithm".into());
    }

    // Create the message to verify: header.payload
    let message = format!(
        "{}.{}",
        BASE64_URL_SAFE.encode(header),
        BASE64_URL_SAFE.encode(payload)
    );

    // Create HMAC-SHA256 instance
    let mut mac = Hmac::<Sha256>::new_from_slice(secret)?;
    mac.update(message.as_bytes());

    // Get the computed HMAC
    let computed_signature = mac.finalize().into_bytes();
    let bytes = computed_signature.as_slice();

    if bytes != signature {
        return Err(Box::new(JWTError {
            reason: "Invalid signature".to_string(),
        })
        .into());
    }

    let parsed_payload: JwtPayload = serde_json::from_slice(&payload)?;

    // Validate expiration (if the `exp` claim exists)
    if let Some(expiration) = parsed_payload.exp {
        // Get current time as Unix timestamp (seconds since epoch)
        let current_time: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64;

        if current_time > expiration {
            Err(Box::new(JWTError {
                reason: "JWT has expired".to_string(),
            })
            .into())
        } else {
            Ok(true)
        }
    } else {
        Err(Box::new(JWTError {
            reason: "JWT does not contain exp field".to_string(),
        })
        .into())
    }
}

pub fn decode_jwt<T>(jwt: &str, secret: &str) -> Result<T, Box<dyn Error>>
where
    T: for<'de> Deserialize<'de>, // T must be deserializable
{
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err("Invalid JWT format".into());
    }

    // Decode header and payload (first and second parts of the JWT)
    let header = BASE64_URL_SAFE.decode(parts[0])?;
    let payload = BASE64_URL_SAFE.decode(parts[1])?;
    let signature = BASE64_URL_SAFE.decode(parts[2])?;

    // Parse the header and payload into their respective types
    let parsed_header: JwtHeader = serde_json::from_slice(&header)?;
    let parsed_payload: T = serde_json::from_slice(&payload)?;

    match validate_signature(
        parsed_header.alg.as_str(),
        &header,
        &payload,
        &signature,
        secret.as_bytes(),
    ) {
        Ok(valid) => {
            if valid {
                Ok(parsed_payload)
            } else {
                Err(Box::new(JWTError {
                    reason: "Invalid JWT string".to_string(),
                })
                .into())
            }
        }
        Err(e) => Err(e),
    }
}
