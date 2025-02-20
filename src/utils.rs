#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        traefik_wasm_api::send_log(traefik_wasm_api::DEBUG, format!($($arg)*).as_str());
    }
}

#[macro_export]
macro_rules! info_log {
    ($($arg:tt)*) => {
        traefik_wasm_api::send_log(traefik_wasm_api::INFO, format!($($arg)*).as_str());
    }
}

#[macro_export]
macro_rules! warn_log {
    ($($arg:tt)*) => {
        traefik_wasm_api::send_log(traefik_wasm_api::WARN, format!($($arg)*).as_str());
    }
}

#[macro_export]
macro_rules! error_log {
    ($($arg:tt)*) => {
        traefik_wasm_api::send_log(traefik_wasm_api::ERROR, format!($($arg)*).as_str());
    }
}
