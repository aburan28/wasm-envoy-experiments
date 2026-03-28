use proxy_wasm::traits::{Context, HttpContext};
use proxy_wasm::types::Action;

use crate::config::PluginConfig;

pub struct ResponseCaptureHttp {
    context_id: u32,
    config: PluginConfig,
    request_path: Option<String>,
    request_method: Option<String>,
    response_status: Option<u16>,
    response_headers: Vec<(String, String)>,
    response_body: Vec<u8>,
    should_capture: bool,
}

impl ResponseCaptureHttp {
    pub fn new(context_id: u32, config: PluginConfig) -> Self {
        Self {
            context_id,
            config,
            request_path: None,
            request_method: None,
            response_status: None,
            response_headers: Vec::new(),
            response_body: Vec::new(),
            should_capture: false,
        }
    }

    fn path_matches(&self, path: &str) -> bool {
        if self.config.path_prefixes.is_empty() {
            return true;
        }
        self.config
            .path_prefixes
            .iter()
            .any(|prefix| path.starts_with(prefix.as_str()))
    }

    fn status_matches(&self, status: u16) -> bool {
        if self.config.status_codes.is_empty() {
            return true;
        }
        self.config.status_codes.contains(&status)
    }

    fn log_capture(&self) {
        let path = self.request_path.as_deref().unwrap_or("-");
        let method = self.request_method.as_deref().unwrap_or("-");
        let status = self.response_status.unwrap_or(0);

        let body_str = if self.config.capture_body && !self.response_body.is_empty() {
            let body = if self.config.max_body_bytes > 0 {
                &self.response_body[..self.response_body.len().min(self.config.max_body_bytes)]
            } else {
                &self.response_body
            };
            match std::str::from_utf8(body) {
                Ok(s) => s.to_string(),
                Err(_) => format!("<binary {} bytes>", body.len()),
            }
        } else {
            String::new()
        };

        if self.config.output_format == "json" {
            let headers_json: Vec<String> = if self.config.capture_headers {
                self.response_headers
                    .iter()
                    .map(|(k, v)| format!("\"{}\":\"{}\"", k, v.replace('"', "\\\"")))
                    .collect()
            } else {
                Vec::new()
            };

            log::info!(
                "[ctx={}] {{\"method\":\"{}\",\"path\":\"{}\",\"status\":{},\"body_bytes\":{},\"headers\":{{{}}},\"body\":\"{}\"}}",
                self.context_id,
                method,
                path.replace('"', "\\\""),
                status,
                self.response_body.len(),
                headers_json.join(","),
                body_str.replace('"', "\\\""),
            );
        } else {
            log::info!(
                "[ctx={}] {} {} -> {} ({} bytes)",
                self.context_id,
                method,
                path,
                status,
                self.response_body.len(),
            );
            if self.config.capture_headers {
                for (k, v) in &self.response_headers {
                    log::info!("[ctx={}]   {}: {}", self.context_id, k, v);
                }
            }
            if !body_str.is_empty() {
                log::info!("[ctx={}]   body: {}", self.context_id, body_str);
            }
        }
    }
}

impl Context for ResponseCaptureHttp {}

impl HttpContext for ResponseCaptureHttp {
    fn on_http_request_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        let path = self.get_http_request_header(":path").unwrap_or_default();
        if self.path_matches(&path) {
            self.request_path = Some(path);
            self.request_method = self.get_http_request_header(":method");
            self.should_capture = true;
        }
        Action::Continue
    }

    fn on_http_response_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        if !self.should_capture {
            return Action::Continue;
        }

        let status: u16 = self
            .get_http_response_header(":status")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        if !self.status_matches(status) {
            self.should_capture = false;
            return Action::Continue;
        }

        self.response_status = Some(status);

        if self.config.capture_headers {
            self.response_headers = self.get_http_response_headers();
        }

        self.add_http_response_header(&self.config.capture_tag_header.clone(), "true");

        Action::Continue
    }

    fn on_http_response_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        if !self.should_capture || !self.config.capture_body {
            if end_of_stream && self.should_capture {
                self.log_capture();
            }
            return Action::Continue;
        }

        if let Some(chunk) = self.get_http_response_body(0, body_size) {
            self.response_body.extend_from_slice(&chunk);
        }

        if end_of_stream {
            self.log_capture();
        }

        Action::Continue
    }
}
