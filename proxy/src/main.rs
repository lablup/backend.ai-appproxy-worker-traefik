use base64::{prelude::BASE64_STANDARD, Engine};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::cell::RefCell;

use appproxy_traefik_lib::{
    cookie, debug_log, dto::SerializableCircuit, error_log, info_log, jwt, warn_log,
};

use traefik_wasm_api;

const CONTINUE_EXECUTION: i64 = 16 << 32 | 1;
const STOP_EXECUTION: i64 = 0 << 32 | 0;

pub struct PluginContext {
    pub config: PluginConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawPluginConfig {
    pub circuit: String, // value contains JSON String here
    pub jwt_secret: String,
    pub permit_hash_secret: String,
    pub permit_cookie_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub circuit: SerializableCircuit,
    pub jwt_secret: String,
    pub permit_hash_secret: String,
    pub permit_cookie_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub id: String,
}

thread_local! {
    static CONFIG: RefCell<Option<PluginConfig>> = RefCell::new(None);
}

fn get_config() -> PluginConfig {
    CONFIG.with(|config_cell| {
        let mut config_opt = config_cell.borrow_mut();
        if config_opt.is_none() {
            let raw_config =
                match serde_json::from_slice::<RawPluginConfig>(&traefik_wasm_api::get_conf()) {
                    Ok(raw_config) => raw_config,
                    Err(e) => {
                        error_log!("Failed to load configuration: {:?}", e);
                        panic!("Error: {:?}", e)
                    }
                };

            let config = PluginConfig {
                circuit: match serde_json::from_slice(raw_config.circuit.as_bytes()) {
                    Ok(c) => c,
                    Err(e) => {
                        error_log!("Failed to load configuration: {:?}", e);
                        panic!("Error: {:?}", e)
                    }
                },
                jwt_secret: raw_config.jwt_secret,
                permit_hash_secret: raw_config.permit_hash_secret,
                permit_cookie_name: raw_config.permit_cookie_name,
            };

            info_log!("Loaded configuration");
            debug_log!("{:?}", config);
            *config_opt = Some(config);
        }
        config_opt.as_ref().unwrap().clone()
    })
}

#[derive(Debug, Clone)]
pub struct AuthenticationError {
    pub reason: String,
}

impl std::fmt::Display for AuthenticationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AuthenticationError: {}", self.reason)
    }
}

#[derive(Debug, Clone)]
pub struct RedisError {
    pub reason: String,
}

impl std::fmt::Display for RedisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RedisError: {}", self.reason)
    }
}

pub fn validate_jwt(circuit_id: &str) -> Result<(), AuthenticationError> {
    let headers =
        traefik_wasm_api::get_header_val(traefik_wasm_api::REQUEST_HEADER, &"Authorization");
    let auth_header = match headers.get(0) {
        Some(h) => h,
        None => {
            return Err(AuthenticationError {
                reason: "Authorization header not found".to_string(),
            });
        }
    };
    let headers: Vec<&str> = auth_header.split(' ').collect();
    match &headers[..] {
        [method, token_str] if *method == "Bearer" || *method == "BackendAI" => {
            debug_log!("Token: {}", token_str);
            let config = get_config();
            match jwt::decode_jwt::<Claims>(&token_str, &config.jwt_secret) {
                Ok(claims) => {
                    if &claims.id == circuit_id {
                        Ok(())
                    } else {
                        Err(AuthenticationError {
                            reason: "JWT does not belong to the circuit ".to_string(),
                        })
                    }
                }
                Err(e) => Err(AuthenticationError {
                    reason: format!("Falied to validate JWT: {:?}", e).to_string(),
                }),
            }
        }
        _ => Err(AuthenticationError {
            reason: "Invalid authentication method".to_string(),
        }),
    }
}

pub fn to_hex_string(bytes: &[u8]) -> String {
    let strs: Vec<String> = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    strs.join("")
}

fn validate_cookie(user_id: &str) -> Result<(), AuthenticationError> {
    let headers = traefik_wasm_api::get_header_val(traefik_wasm_api::REQUEST_HEADER, &"Cookie");

    let Some(cookie_str) = headers.get(0) else {
        return Err(AuthenticationError {
            reason: "Cookie header not found".to_string(),
        });
    };

    let cookie_map = cookie::parse_cookie_header(&cookie_str);

    let config = get_config();
    let provided_hash = match cookie_map.get(config.permit_cookie_name.as_str()) {
        Some(h) => h,
        None => {
            return Err(AuthenticationError {
                reason: "Permit cookie not found from cookie jar".to_string(),
            });
        }
    };
    debug_log!(
        "Permit hash: {} ({})",
        config.permit_hash_secret,
        BASE64_STANDARD.encode(config.permit_hash_secret.as_bytes())
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(config.permit_hash_secret.as_bytes())
        .expect("Should not reach here");
    mac.update(user_id.as_bytes());
    debug_log!(
        "Hashing {} ({})",
        user_id,
        BASE64_STANDARD.encode(user_id.as_bytes())
    );
    let digest = to_hex_string(mac.finalize().into_bytes().as_slice());
    debug_log!("Digest: {}", digest);
    let calculated_hash = BASE64_STANDARD.encode(digest.as_bytes());
    if provided_hash.as_str() == calculated_hash.as_str() {
        Ok(())
    } else {
        Err(AuthenticationError {
            reason: format!(
                "Invalid hash (expected {}, got {})",
                calculated_hash, provided_hash
            )
            .to_string(),
        })
    }
}

fn handle_preflight_request() {
    traefik_wasm_api::set_code(204);
}

#[export_name = "handle_request"]
pub fn http_request() -> i64 {
    let path = traefik_wasm_api::get_request_uri();

    info_log!("{:?} start", path);

    if traefik_wasm_api::get_request_method() == "OPTIONS" {
        handle_preflight_request();
        return 0;
    }

    let config = get_config();
    let skip_auth = config.circuit.open_to_public;

    let control = if !skip_auth {
        match (
            config.circuit.endpoint_id.clone(),
            config.circuit.user_id.clone(),
        ) {
            (Some(_), _) => match validate_jwt(config.circuit.id.as_str()) {
                Ok(_) => CONTINUE_EXECUTION,
                Err(e) => {
                    warn_log!("Failed to authenticate JWT: {:?}", e);
                    traefik_wasm_api::set_code(401);
                    traefik_wasm_api::set_header(
                        traefik_wasm_api::RESPONSE_HEADER,
                        &"Content-Type",
                        "text/plain",
                    );
                    traefik_wasm_api::writebody(
                        traefik_wasm_api::RESPONSE_BODY,
                        "Not Authenticated - INVALIDJWT",
                    );
                    // FIXME: indicate about the error to client
                    STOP_EXECUTION
                }
            },
            (None, Some(user_id)) => match validate_cookie(user_id.as_str()) {
                Ok(_) => CONTINUE_EXECUTION,
                Err(e) => {
                    warn_log!("Failed to authenticate cookie: {:?}", e);
                    traefik_wasm_api::set_code(401);
                    traefik_wasm_api::set_header(
                        traefik_wasm_api::RESPONSE_HEADER,
                        &"Content-Type",
                        "text/plain",
                    );
                    traefik_wasm_api::writebody(
                        traefik_wasm_api::RESPONSE_BODY,
                        "Not Authenticated - INVALIDCOOKIE",
                    );

                    STOP_EXECUTION
                }
            },
            (None, None) => {
                // FIXME: indicate about the error to client
                traefik_wasm_api::set_code(401);
                traefik_wasm_api::set_header(
                    traefik_wasm_api::RESPONSE_HEADER,
                    &"Content-Type",
                    "text/plain",
                );
                traefik_wasm_api::writebody(
                    traefik_wasm_api::RESPONSE_BODY,
                    "Not Authenticated - NOCIRCUIT(N)",
                );
                STOP_EXECUTION
            }
        }
    } else {
        CONTINUE_EXECUTION
    };

    if control != CONTINUE_EXECUTION {
        return control;
    }

    let Ok(req_ctx_u32) = getrandom::u32() else {
        panic!("RNG not working!")
    };
    let req_ctx = req_ctx_u32 as i32;

    (req_ctx as i64) << 32 | 1
}

#[export_name = "handle_response"]
fn http_response(_req_ctx: i32, _is_error: i32) {
    let path = traefik_wasm_api::get_request_uri();

    info_log!("{:?} complete", path);
}

fn main() {}
