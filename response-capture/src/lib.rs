mod config;
mod http_context;
mod root_context;

use proxy_wasm::traits::RootContext;
use proxy_wasm::types::LogLevel;

use crate::config::PluginConfig;
use crate::root_context::ResponseCaptureRoot;

proxy_wasm::main! {{
    proxy_wasm::set_log_level(LogLevel::Info);
    proxy_wasm::set_root_context(|_| -> Box<dyn RootContext> {
        Box::new(ResponseCaptureRoot::new(PluginConfig::default()))
    });
}}
