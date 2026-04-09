use proxy_wasm::traits::{Context, HttpContext, RootContext};
use proxy_wasm::types::ContextType;

use crate::config::PluginConfig;
use crate::http_context::ProtoFilterHttp;

pub struct ProtoFilterRoot {
    config: PluginConfig,
}

impl ProtoFilterRoot {
    pub fn new(config: PluginConfig) -> Self {
        Self { config }
    }
}

impl Context for ProtoFilterRoot {}

impl RootContext for ProtoFilterRoot {
    fn on_configure(&mut self, _plugin_configuration_size: usize) -> bool {
        if let Some(config_bytes) = self.get_plugin_configuration() {
            match serde_json::from_slice::<PluginConfig>(&config_bytes) {
                Ok(config) => {
                    log::info!("wasm-proto-filter configured: {:?}", config);
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
        Some(Box::new(ProtoFilterHttp::new(
            context_id,
            self.config.clone(),
        )))
    }

    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }
}
