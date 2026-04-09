use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct PluginConfig {
    /// Envoy cluster name for the ext_proc gRPC service
    #[serde(default = "default_cluster")]
    pub ext_proc_cluster: String,

    /// Fully-qualified gRPC service name on the ext_proc endpoint
    #[serde(default = "default_service")]
    pub ext_proc_service: String,

    /// gRPC method name on the ext_proc endpoint
    #[serde(default = "default_method")]
    pub ext_proc_method: String,

    /// Timeout for ext_proc calls in milliseconds
    #[serde(default = "default_timeout")]
    pub ext_proc_timeout_ms: u64,

    /// Process request bodies through ext_proc
    #[serde(default = "default_true")]
    pub process_request: bool,

    /// Process response bodies through ext_proc
    #[serde(default = "default_true")]
    pub process_response: bool,

    /// Only process these gRPC services (empty = all)
    #[serde(default)]
    pub services: Vec<String>,

    /// Only process these gRPC methods (empty = all)
    #[serde(default)]
    pub methods: Vec<String>,

    /// Max proto payload bytes to send to ext_proc (0 = unlimited)
    #[serde(default)]
    pub max_payload_bytes: usize,

    /// If true, continue on ext_proc failure instead of rejecting
    #[serde(default = "default_true")]
    pub fail_open: bool,
}

fn default_cluster() -> String {
    "ext_proc_cluster".to_string()
}

fn default_service() -> String {
    "proto_mutation.v1.ProtoMutationService".to_string()
}

fn default_method() -> String {
    "ProcessMessage".to_string()
}

fn default_timeout() -> u64 {
    200
}

fn default_true() -> bool {
    true
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            ext_proc_cluster: default_cluster(),
            ext_proc_service: default_service(),
            ext_proc_method: default_method(),
            ext_proc_timeout_ms: default_timeout(),
            process_request: true,
            process_response: true,
            services: Vec::new(),
            methods: Vec::new(),
            max_payload_bytes: 0,
            fail_open: true,
        }
    }
}
