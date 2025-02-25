use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HostPortPair {
    pub host: String,
    pub port: i32,
}
