use proxy_wasm::traits::{Context, HttpContext, RootContext};
use proxy_wasm::types::ContextType;

use crate::config::PluginConfig;
use crate::http_context::ResponseCaptureHttp;

pub struct ResponseCaptureRoot {
    config: PluginConfig,
}

impl ResponseCaptureRoot {
    pub fn new(config: PluginConfig) -> Self {
        Self { config }
    }
}

impl Context for ResponseCaptureRoot {}

impl RootContext for ResponseCaptureRoot {
    fn on_configure(&mut self, _plugin_configuration_size: usize) -> bool {
        if let Some(config_bytes) = self.get_plugin_configuration() {
            match serde_json::from_slice::<PluginConfig>(&config_bytes) {
                Ok(config) => {
                    log::info!("response-capture configured: {:?}", config);
                    self.config = config;
                }
                Err(e) => {
                    log::error!("failed to parse plugin config: {}", e);
                    return false;
                }
            }
        }
        true
    }

    fn create_http_context(&self, context_id: u32) -> Option<Box<dyn HttpContext>> {
        Some(Box::new(ResponseCaptureHttp::new(
            context_id,
            self.config.clone(),
        )))
    }

    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }
}
