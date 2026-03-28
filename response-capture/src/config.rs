use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct PluginConfig {
    /// HTTP status codes to capture (empty = all)
    #[serde(default)]
    pub status_codes: Vec<u16>,

    /// Path prefixes to capture (empty = all)
    #[serde(default)]
    pub path_prefixes: Vec<String>,

    /// Capture response headers
    #[serde(default = "default_true")]
    pub capture_headers: bool,

    /// Capture response body
    #[serde(default = "default_true")]
    pub capture_body: bool,

    /// Max response body bytes to capture (0 = unlimited)
    #[serde(default)]
    pub max_body_bytes: usize,

    /// Custom header to tag captured responses (value is set to "true" on matched responses)
    #[serde(default = "default_capture_header")]
    pub capture_tag_header: String,

    /// Log output format: "json" or "plain"
    #[serde(default = "default_format")]
    pub output_format: String,
}

fn default_true() -> bool {
    true
}

fn default_capture_header() -> String {
    "x-response-captured".to_string()
}

fn default_format() -> String {
    "json".to_string()
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            status_codes: Vec::new(),
            path_prefixes: Vec::new(),
            capture_headers: true,
            capture_body: true,
            max_body_bytes: 0,
            capture_tag_header: default_capture_header(),
            output_format: default_format(),
        }
    }
}
