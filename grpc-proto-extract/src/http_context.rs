use proxy_wasm::traits::{Context, HttpContext};
use proxy_wasm::types::Action;

use crate::config::PluginConfig;
use crate::grpc;
use crate::proto_decode;

pub struct GrpcExtractHttp {
    context_id: u32,
    config: PluginConfig,
    grpc_path: Option<String>,
    request_body: Vec<u8>,
    response_body: Vec<u8>,
}

impl GrpcExtractHttp {
    pub fn new(context_id: u32, config: PluginConfig) -> Self {
        Self {
            context_id,
            config,
            grpc_path: None,
            request_body: Vec::new(),
            response_body: Vec::new(),
        }
    }

    fn should_capture(&self, service: &str, method: &str) -> bool {
        let service_match =
            self.config.services.is_empty() || self.config.services.iter().any(|s| s == service);
        let method_match =
            self.config.methods.is_empty() || self.config.methods.iter().any(|m| m == method);
        service_match && method_match
    }

    fn log_proto(&self, direction: &str, path: &str, payload: &[u8]) {
        let fields = proto_decode::decode_raw(payload);
        log::info!(
            "[ctx={}] {} {} | grpc_frames=decoded | fields={:?}",
            self.context_id,
            direction,
            path,
            fields
        );
    }
}

impl Context for GrpcExtractHttp {}

impl HttpContext for GrpcExtractHttp {
    fn on_http_request_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        let content_type = self
            .get_http_request_header("content-type")
            .unwrap_or_default();
        if !content_type.starts_with("application/grpc") {
            return Action::Continue;
        }

        let path = self.get_http_request_header(":path").unwrap_or_default();
        if let Some((service, method)) = grpc::parse_grpc_path(&path) {
            if self.should_capture(service, method) {
                log::info!(
                    "[ctx={}] gRPC request: {}/{}",
                    self.context_id,
                    service,
                    method
                );
                self.grpc_path = Some(path);
            }
        }

        Action::Continue
    }

    fn on_http_request_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        if self.grpc_path.is_none() || !self.config.capture_request {
            return Action::Continue;
        }

        if let Some(chunk) = self.get_http_request_body(0, body_size) {
            self.request_body.extend_from_slice(&chunk);
        }

        if end_of_stream {
            if let Some(path) = &self.grpc_path {
                let path = path.clone();
                for frame in grpc::parse_grpc_frames(&self.request_body) {
                    let payload = if self.config.max_payload_bytes > 0 {
                        &frame[..frame.len().min(self.config.max_payload_bytes)]
                    } else {
                        &frame
                    };
                    self.log_proto("REQUEST", &path, payload);
                }
            }
            self.request_body.clear();
        }

        Action::Continue
    }

    fn on_http_response_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        Action::Continue
    }

    fn on_http_response_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        if self.grpc_path.is_none() || !self.config.capture_response {
            return Action::Continue;
        }

        if let Some(chunk) = self.get_http_response_body(0, body_size) {
            self.response_body.extend_from_slice(&chunk);
        }

        if end_of_stream {
            if let Some(path) = &self.grpc_path {
                let path = path.clone();
                for frame in grpc::parse_grpc_frames(&self.response_body) {
                    let payload = if self.config.max_payload_bytes > 0 {
                        &frame[..frame.len().min(self.config.max_payload_bytes)]
                    } else {
                        &frame
                    };
                    self.log_proto("RESPONSE", &path, payload);
                }
            }
            self.response_body.clear();
        }

        Action::Continue
    }
}
