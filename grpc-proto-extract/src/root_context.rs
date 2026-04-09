use proxy_wasm::traits::{Context, HttpContext, RootContext};
use proxy_wasm::types::ContextType;

use crate::config::PluginConfig;
use crate::http_context::GrpcExtractHttp;

pub struct GrpcExtractRoot {
    config: PluginConfig,
}

impl GrpcExtractRoot {
    pub fn new(config: PluginConfig) -> Self {
        Self { config }
    }
}

impl Context for GrpcExtractRoot {}

impl RootContext for GrpcExtractRoot {
    fn on_configure(&mut self, _plugin_configuration_size: usize) -> bool {
        if let Some(config_bytes) = self.get_plugin_configuration() {
            match serde_json::from_slice::<PluginConfig>(&config_bytes) {
                Ok(config) => {
                    log::info!("grpc-proto-extract configured: {:?}", config);
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
        Some(Box::new(GrpcExtractHttp::new(
            context_id,
            self.config.clone(),
        )))
    }

    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }
}
