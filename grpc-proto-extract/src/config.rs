use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct PluginConfig {
    /// Log request bodies
    #[serde(default = "default_true")]
    pub capture_request: bool,

    /// Log response bodies
    #[serde(default = "default_true")]
    pub capture_response: bool,

    /// Only capture these gRPC services (empty = all)
    #[serde(default)]
    pub services: Vec<String>,

    /// Only capture these gRPC methods (empty = all)
    #[serde(default)]
    pub methods: Vec<String>,

    /// Max proto payload bytes to decode (0 = unlimited)
    #[serde(default)]
    pub max_payload_bytes: usize,
}

fn default_true() -> bool {
    true
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            capture_request: true,
            capture_response: true,
            services: Vec::new(),
            methods: Vec::new(),
            max_payload_bytes: 0,
        }
    }
}
