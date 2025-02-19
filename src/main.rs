use base64::{prelude::BASE64_STANDARD, Engine};
use hmac::{Hmac, Mac};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
mod cookie;
mod jwt;

const CONTINUE_EXECUTION: i64 = 16 << 32 | 1;
const STOP_EXECUTION: i64 = 0 << 32 | 0;
// class FrontendMode(str, enum.Enum):

// class ProxyProtocol(str, enum.Enum):
//     HTTP = "http"
//     GRPC = "grpc"
//     HTTP2 = "h2"
//     TCP = "tcp"
//     PREOPEN = "preopen"

// class AppMode(str, enum.Enum):
//     INTERACTIVE = "interactive"
//     INFERENCE = "inference"

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FrontendMode {
    WildcardDomain,
    Port,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProxyProtocol {
    Http,
    Grpc,
    Http2,
    Tcp,
    Preopen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppMode {
    Interactive,
    Inference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    // route_id: Annotated[UUID | None, Field(default=None)]
    // session_id: UUID
    // session_name: Annotated[str | None, Field(default=None)]
    // kernel_host: str
    // kernel_port: int
    // protocol: ProxyProtocol
    // traffic_ratio: Annotated[float, Field(default=1.0)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginContext {
    pub config: PluginConfig,
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

// type HmacSha256 = Hmac<Sha256>;

use traefik_wasm_api;

// Returns the Structured Configuration from the host in JSON format
lazy_static! {
    static ref CONFIG: PluginContext = {
        match serde_json::from_slice(&traefik_wasm_api::get_conf()) {
            Ok(config) => {
                traefik_wasm_api::send_log(
                    traefik_wasm_api::DEBUG,
                    format!("{:?}", config).as_str(),
                );
                PluginContext { config: config }
            }
            Err(e) => {
                let empty_config = PluginContext {
                    config: PluginConfig {
                        circuits: HashMap::new(),
                        host_circuit_map: HashMap::new(),
                        permit_hash_secret: "".to_string(),
                        permit_cookie_name: "appproxy_permit".to_string(),
                        jwt_secret: "".to_string(),
                    },
                };
                traefik_wasm_api::send_log(traefik_wasm_api::WARN, format!("{:?}", e).as_str());
                traefik_wasm_api::send_log(
                    traefik_wasm_api::WARN,
                    format!("{:?}", empty_config).as_str(),
                );
                empty_config
            }
        }
    };
}

#[derive(Debug, Clone)]
pub struct AuthenticationError;

pub fn validate_jwt(circuit_id: &str) -> Result<(), AuthenticationError> {
    let headers =
        traefik_wasm_api::get_header_val(traefik_wasm_api::REQUEST_HEADER, &"Authorization");
    let auth_header = match headers.get(0) {
        Some(h) => h,
        None => {
            return Err(AuthenticationError);
        }
    };
    let headers: Vec<&str> = auth_header.split(' ').collect();
    match &headers[..] {
        [method, token_str] if *method == "Bearer" || *method == "BackendAI" => {
            match jwt::decode_jwt::<Claims>(&token_str, &CONFIG.config.jwt_secret) {
                Ok(claims) => {
                    if &claims.id == circuit_id {
                        Ok(())
                    } else {
                        Err(AuthenticationError)
                    }
                }
                Err(e) => Err(AuthenticationError),
            }
        }
        _ => Err(AuthenticationError),
    }
}

fn validate_cookie(user_id: &str) -> Result<(), AuthenticationError> {
    let headers = traefik_wasm_api::get_header_val(traefik_wasm_api::REQUEST_HEADER, &"Cookies");

    let Some(cookie_str) = headers.get(0) else {
        return Err(AuthenticationError);
    };

    let cookie_map = cookie::parse_cookie_header(&cookie_str);

    let provided_hash = match cookie_map.get(CONFIG.config.permit_cookie_name.as_str()) {
        Some(h) => h,
        None => {
            return Err(AuthenticationError);
        }
    };
    let mut mac = Hmac::<Sha256>::new_from_slice(CONFIG.config.permit_hash_secret.as_bytes())
        .expect("Should not reach here");
    mac.update(user_id.as_bytes());
    let calculated_hash = BASE64_STANDARD.encode(mac.finalize().into_bytes());
    if provided_hash.as_str() == calculated_hash.as_str() {
        Ok(())
    } else {
        Err(AuthenticationError)
    }
}

#[export_name = "handle_request"]
pub fn http_request() -> i64 {
    let addr = traefik_wasm_api::get_addr();
    let path = traefik_wasm_api::get_request_uri();
    let headers = traefik_wasm_api::get_header_val(traefik_wasm_api::REQUEST_HEADER, &"Host");
    let host = match headers.get(0) {
        Some(h) => h,
        None => {
            // FIXME: indicate about the error to client
            return STOP_EXECUTION;
        }
    };

    traefik_wasm_api::send_log(traefik_wasm_api::INFO, format!("{:?} start", path).as_str());
    traefik_wasm_api::send_log(
        traefik_wasm_api::INFO,
        format!("{:?} {:?}", host, addr).as_str(),
    );

    let circuit = match CONFIG.config.host_circuit_map.get(host) {
        Some(i) => match CONFIG.config.circuits.get(i) {
            Some(c) => c,
            None => {
                // FIXME: indicate about the error to client
                return STOP_EXECUTION;
            }
        },
        None => {
            // FIXME: indicate about the error to client
            return STOP_EXECUTION;
        }
    };

    if circuit.open_to_public {
        return CONTINUE_EXECUTION;
    }

    match (circuit.endpoint_id.clone(), circuit.user_id.clone()) {
        (Some(_), Some(_)) => {
            traefik_wasm_api::writebody(traefik_wasm_api::RESPONSE_BODY, "Not Authenticated");
            traefik_wasm_api::set_code(401);
            // FIXME: indicate about the error to client
            STOP_EXECUTION
        }
        (Some(endpoint_id), None) => match validate_jwt(circuit.id.as_str()) {
            Ok(_) => CONTINUE_EXECUTION,
            Err(_) => {
                traefik_wasm_api::writebody(traefik_wasm_api::RESPONSE_BODY, "Not Authenticated");
                traefik_wasm_api::set_code(401);
                // FIXME: indicate about the error to client
                STOP_EXECUTION
            }
        },
        (None, Some(user_id)) => match validate_cookie(user_id.as_str()) {
            Ok(_) => CONTINUE_EXECUTION,
            Err(_) => {
                // FIXME: indicate about the error to client
                traefik_wasm_api::writebody(traefik_wasm_api::RESPONSE_BODY, "Not Authenticated");
                traefik_wasm_api::set_code(401);

                STOP_EXECUTION
            }
        },
        (None, None) => {
            // FIXME: indicate about the error to client
            traefik_wasm_api::writebody(traefik_wasm_api::RESPONSE_BODY, "Not Authenticated");
            traefik_wasm_api::set_code(401);
            STOP_EXECUTION
        }
    }
}

#[export_name = "handle_response"]
fn http_response(_req_ctx: i32, _is_error: i32) {
    let path = traefik_wasm_api::get_request_uri();
    traefik_wasm_api::send_log(
        traefik_wasm_api::INFO,
        format!("{:?} complete", path).as_str(),
    );

    traefik_wasm_api::send_log(traefik_wasm_api::INFO, format!("{:?}", _req_ctx).as_str())
}

fn main() {}
