use base64::{prelude::BASE64_STANDARD, Engine};
use hmac::{Hmac, Mac};
use lazy_static::lazy_static;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::Sha256;
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
mod cookie;
mod jwt;
mod utils;
use std::path::Path;
use traefik_wasm_api;

const CONTINUE_EXECUTION: i64 = 16 << 32 | 1;
const STOP_EXECUTION: i64 = 0 << 32 | 0;
const LAST_USED_TIME_MARKER_PATH: &str = "/appproxy-traefik/last-used-time-marker";

#[derive(Debug, Clone, Serialize)]
pub enum FrontendMode {
    WildcardDomain,
    Port,
}
impl<'de> serde::Deserialize<'de> for FrontendMode {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let variant = String::deserialize(de)?;
        Ok(match variant.as_str() {
            "wildcard" => FrontendMode::WildcardDomain,
            "port" => FrontendMode::Port,
            _ => todo!("should not reach here"),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum ProxyProtocol {
    Http,
    Grpc,
    Http2,
    Tcp,
    Preopen,
}

impl<'de> serde::Deserialize<'de> for ProxyProtocol {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let variant = String::deserialize(de)?;
        Ok(match variant.as_str() {
            "http" => ProxyProtocol::Http,
            "grpc" => ProxyProtocol::Grpc,
            "http2" => ProxyProtocol::Http2,
            "tcp" => ProxyProtocol::Tcp,
            "preopen" => ProxyProtocol::Preopen,
            _ => todo!("should not reach here"),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum AppMode {
    Interactive,
    Inference,
}

impl<'de> serde::Deserialize<'de> for AppMode {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let variant = String::deserialize(de)?;
        Ok(match variant.as_str() {
            "interactive" => AppMode::Interactive,
            "inference" => AppMode::Inference,
            _ => todo!("should not reach here"),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    pub route_id: Option<String>,
    pub session_id: String,
    pub session_name: Option<String>,
    pub kernel_host: String,
    pub kernel_port: i32,
    pub protocol: ProxyProtocol,
    pub traffic_ratio: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableCircuit {
    pub id: String,
    pub app: String,
    pub protocol: ProxyProtocol,
    pub worker: String,
    pub app_mode: AppMode,
    pub frontend_mode: FrontendMode,
    pub envs: HashMap<String, String>, // This represents a dictionary of string keys and values of any type.
    pub arguments: Option<String>,
    pub open_to_public: bool,
    pub allowed_client_ips: Option<String>,
    pub port: Option<i32>,
    pub subdomain: Option<String>,
    pub user_id: Option<String>,
    pub endpoint_id: Option<String>,
    pub runtime_variant: Option<String>,
    pub route_info: Vec<RouteInfo>,
    pub session_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct PluginContext {
    pub config: PluginConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HostPortPair {
    pub host: String,
    pub port: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawPluginConfig {
    pub circuits: HashMap<String, String>, // value contains JSON String here
    pub host_circuit_map: HashMap<String, String>,
    pub jwt_secret: String,
    pub permit_hash_secret: String,
    pub permit_cookie_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginConfig {
    pub circuits: HashMap<String, SerializableCircuit>,
    pub host_circuit_map: HashMap<String, String>,
    pub jwt_secret: String,
    pub permit_hash_secret: String,
    pub permit_cookie_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub id: String,
}

// Returns the Structured Configuration from the host in JSON format
lazy_static! {
    static ref CONTEXT: Mutex<HashMap<i32, String>> = Mutex::new(HashMap::new());
    static ref CONFIG: PluginConfig = {
        if !Path::new(LAST_USED_TIME_MARKER_PATH).exists() {
            error_log!("last-used-time-marker file not found");
            panic!("Initialization failed")
        }
        let paths = fs::read_dir("/appproxy-traefik").unwrap();

        for path in paths {
            debug_log!("Name: {}", path.unwrap().path().display());
        }

        let raw_config =
            match serde_json::from_slice::<RawPluginConfig>(&traefik_wasm_api::get_conf()) {
                Ok(raw_config) => raw_config,
                Err(e) => {
                    error_log!("Failed to load configuration: {:?}", e);
                    panic!("Error: {:?}", e)
                }
            };

        let mut config = PluginConfig {
            circuits: HashMap::new(),
            host_circuit_map: HashMap::new(),
            jwt_secret: raw_config.jwt_secret,
            permit_hash_secret: raw_config.permit_hash_secret,
            permit_cookie_name: raw_config.permit_cookie_name,
        };

        for (key, value) in raw_config.circuits.into_iter() {
            match serde_json::from_slice(value.as_bytes()) {
                Ok(c) => config.circuits.insert(key, c),
                Err(e) => {
                    error_log!("Failed to load configuration: {:?}", e);
                    panic!("Error: {:?}", e)
                }
            };
        }
        for (key, value) in raw_config.host_circuit_map.into_iter() {
            match urlencoding::decode(key.as_str()) {
                Ok(s) => config.host_circuit_map.insert(s.to_string(), value),
                Err(e) => {
                    error_log!("Failed to load configuration: {:?}", e);
                    panic!("Error: {:?}", e)
                }
            };
        }
        info_log!("Loaded configuration");
        debug_log!("{:?}", config);
        config
    };
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

pub fn mark_last_used_time(
    session_ids: Vec<String>,
    circuit_id: &str,
    is_websocket: bool,
) -> Result<(), io::Error> {
    let mut lines: Vec<String> = session_ids
        .iter()
        .map(|id| format!("mark,session.{}.last_access\n", id).to_string())
        .collect();
    lines.push(format!("mark,circuit.{}.last_access\n", circuit_id).to_string());
    if is_websocket {
        lines.push(format!("activate,{}\n", circuit_id).to_string());
    }

    let mut file = OpenOptions::new()
        .write(true)
        .open(LAST_USED_TIME_MARKER_PATH)
        .expect("Failed to open last used time marker");

    for line in lines {
        file.write(line.as_bytes()).unwrap();
        debug_log!("Wrote {}", line);
    }
    file.flush()
}

pub fn clear_websocket_last_used_time_marker(circuit_id: &str) -> Result<(), io::Error> {
    let mut file = OpenOptions::new()
        .write(true)
        .open(LAST_USED_TIME_MARKER_PATH)
        .expect("Failed to open last used time marker");
    let line = format!("deactivate,{}", circuit_id);
    if let Err(e) = file.write(line.as_bytes()) {
        error_log!("Error while recording last used time information: {:?}", e);
    }
    debug_log!("Wrote {}", line);
    file.flush()
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
            match jwt::decode_jwt::<Claims>(&token_str, &CONFIG.jwt_secret) {
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

    let provided_hash = match cookie_map.get(CONFIG.permit_cookie_name.as_str()) {
        Some(h) => h,
        None => {
            return Err(AuthenticationError {
                reason: "Permit cookie not found from cookie jar".to_string(),
            });
        }
    };
    debug_log!(
        "Permit hash: {} ({})",
        CONFIG.permit_hash_secret,
        BASE64_STANDARD.encode(CONFIG.permit_hash_secret.as_bytes())
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(CONFIG.permit_hash_secret.as_bytes())
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

#[export_name = "handle_request"]
pub fn http_request() -> i64 {
    let path = traefik_wasm_api::get_request_uri();
    let host_headers = traefik_wasm_api::get_header_val(traefik_wasm_api::REQUEST_HEADER, &"Host");
    let host = match host_headers.get(0) {
        Some(h) => h,
        None => {
            traefik_wasm_api::writebody(traefik_wasm_api::RESPONSE_BODY, "host header not found");
            traefik_wasm_api::set_code(500);
            return STOP_EXECUTION;
        }
    };

    info_log!("{:?} start", path);

    let circuit = match CONFIG.host_circuit_map.get(host) {
        Some(i) => match CONFIG.circuits.get(i) {
            Some(c) => c,
            None => {
                traefik_wasm_api::writebody(traefik_wasm_api::RESPONSE_BODY, "circuit not found");
                traefik_wasm_api::set_code(500);
                // fixme: indicate about the error to client
                return STOP_EXECUTION;
            }
        },
        None => {
            // FIXME: indicate about the error to client
            traefik_wasm_api::writebody(
                traefik_wasm_api::RESPONSE_BODY,
                format!("Cannot find reference to circuit using host {:?}", host).as_str(),
            );
            traefik_wasm_api::set_code(500);
            return STOP_EXECUTION;
        }
    };

    let control = if !circuit.open_to_public {
        match (circuit.endpoint_id.clone(), circuit.user_id.clone()) {
            (Some(_), _) => match validate_jwt(circuit.id.as_str()) {
                Ok(_) => CONTINUE_EXECUTION,
                Err(e) => {
                    warn_log!("Failed to authenticate JWT: {:?}", e);
                    traefik_wasm_api::writebody(
                        traefik_wasm_api::RESPONSE_BODY,
                        "Not Authenticated - INVALIDJWT",
                    );
                    traefik_wasm_api::set_code(401);
                    // FIXME: indicate about the error to client
                    STOP_EXECUTION
                }
            },
            (None, Some(user_id)) => match validate_cookie(user_id.as_str()) {
                Ok(_) => CONTINUE_EXECUTION,
                Err(e) => {
                    warn_log!("Failed to authenticate cookie: {:?}", e);
                    traefik_wasm_api::writebody(
                        traefik_wasm_api::RESPONSE_BODY,
                        "Not Authenticated - INVALIDCOOKIE",
                    );
                    traefik_wasm_api::set_code(401);

                    STOP_EXECUTION
                }
            },
            (None, None) => {
                // FIXME: indicate about the error to client
                traefik_wasm_api::writebody(
                    traefik_wasm_api::RESPONSE_BODY,
                    "Not Authenticated - NOCIRCUIT(N)",
                );
                traefik_wasm_api::set_code(401);
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
    let upgrade_headers =
        traefik_wasm_api::get_header_val(traefik_wasm_api::REQUEST_HEADER, &"Upgrade");
    let is_websocket = match upgrade_headers.get(0) {
        Some(upgrade) => upgrade.to_lowercase() == "websocket",
        None => false,
    };
    let control = if is_websocket {
        match CONTEXT.lock() {
            Ok(mut context) => {
                context.insert(req_ctx, circuit.id.to_string());
                (req_ctx as i64) << 32 | 1
            }
            Err(e) => {
                error_log!("Failed to set request context: {:?}", e);
                0
            }
        }
    } else {
        control
    };

    debug_log!("Control: {}", control);

    if control != 0 {
        match mark_last_used_time(circuit.session_ids.clone(), &circuit.id, is_websocket) {
            Ok(_) => control,
            Err(e) => {
                error_log!(
                    "Failed to mark last used time for circuit {}: {:?}",
                    circuit.id,
                    e
                );

                if is_websocket {
                    match CONTEXT.lock() {
                        Ok(mut context) => {
                            context.remove(&req_ctx);
                        }
                        Err(e) => {
                            error_log!("Failed to set request context: {:?}", e);
                        }
                    };
                }
                control
            }
        }
    } else {
        0
    }
}

#[export_name = "handle_response"]
fn http_response(_req_ctx: i32, _is_error: i32) {
    let path = traefik_wasm_api::get_request_uri();

    match CONTEXT.lock() {
        Ok(mut context) => {
            if let Some(circuit_id) = context.remove(&_req_ctx) {
                if let Err(e) = clear_websocket_last_used_time_marker(&circuit_id) {
                    error_log!(
                        "Failed to clear active marker for {} on redis: {:?}",
                        circuit_id,
                        e
                    );
                }
            }
        }
        Err(e) => {
            error_log!("Failed to delete request context: {:?}", e);
        }
    }
    info_log!("{:?} complete", path);
}

fn main() {}
