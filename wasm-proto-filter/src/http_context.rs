use std::time::Duration;

use proxy_wasm::traits::{Context, HttpContext};
use proxy_wasm::types::Action;

use crate::config::PluginConfig;
use crate::ext_proc;
use crate::grpc;
use crate::proto_decode;

/// Tracks which direction a pending ext_proc gRPC call belongs to.
enum PendingCall {
    Request(u32),
    Response(u32),
}

pub struct ProtoFilterHttp {
    context_id: u32,
    config: PluginConfig,
    grpc_service: Option<String>,
    grpc_method: Option<String>,
    grpc_path: Option<String>,
    request_body: Vec<u8>,
    response_body: Vec<u8>,
    pending_call: Option<PendingCall>,
}

impl ProtoFilterHttp {
    pub fn new(context_id: u32, config: PluginConfig) -> Self {
        Self {
            context_id,
            config,
            grpc_service: None,
            grpc_method: None,
            grpc_path: None,
            request_body: Vec::new(),
            response_body: Vec::new(),
            pending_call: None,
        }
    }

    fn should_process(&self, service: &str, method: &str) -> bool {
        let service_match =
            self.config.services.is_empty() || self.config.services.iter().any(|s| s == service);
        let method_match =
            self.config.methods.is_empty() || self.config.methods.iter().any(|m| m == method);
        service_match && method_match
    }

    /// Send the proto payload to the external processing service via gRPC.
    /// Returns the call token on success.
    fn dispatch_to_ext_proc(&self, direction: u64, payload: &[u8]) -> Option<u32> {
        let service_name = self.grpc_service.clone().unwrap_or_default();
        let method_name = self.grpc_method.clone().unwrap_or_default();
        let decoded_fields = proto_decode::decode_raw(payload);

        let request_bytes = ext_proc::encode_process_request(
            &service_name,
            &method_name,
            direction,
            payload,
            &decoded_fields,
        );

        let cluster = self.config.ext_proc_cluster.clone();
        let svc = self.config.ext_proc_service.clone();
        let method = self.config.ext_proc_method.clone();
        let timeout = Duration::from_millis(self.config.ext_proc_timeout_ms);

        match self.dispatch_grpc_call(
            &cluster,
            &svc,
            &method,
            Vec::new(),
            Some(&request_bytes),
            timeout,
        ) {
            Ok(token) => {
                log::debug!(
                    "[ctx={}] dispatched ext_proc call, token={}",
                    self.context_id,
                    token
                );
                Some(token)
            }
            Err(e) => {
                log::error!(
                    "[ctx={}] failed to dispatch ext_proc call: {:?}",
                    self.context_id,
                    e
                );
                None
            }
        }
    }

    /// Process the ext_proc response: apply mutations, replace body, or reject.
    /// Returns true if the request was rejected (caller should NOT resume).
    fn apply_response(
        &self,
        response: &ext_proc::ProcessMessageResponse,
        is_request: bool,
    ) -> bool {
        match response.action {
            ext_proc::ACTION_CONTINUE => {
                log::debug!("[ctx={}] ext_proc: continue (no changes)", self.context_id);
            }
            ext_proc::ACTION_MUTATE_FIELDS => {
                let original_body = if is_request {
                    &self.request_body
                } else {
                    &self.response_body
                };

                let frames = grpc::parse_grpc_frames(original_body);
                let mut new_body = Vec::new();
                for frame in frames {
                    let mutated = ext_proc::apply_mutations(frame, &response.mutations);
                    new_body.extend(ext_proc::encode_grpc_frame(&mutated));
                }

                if is_request {
                    self.set_http_request_body(0, original_body.len(), &new_body);
                } else {
                    self.set_http_response_body(0, original_body.len(), &new_body);
                }
                log::info!(
                    "[ctx={}] ext_proc: applied {} mutation(s)",
                    self.context_id,
                    response.mutations.len()
                );
            }
            ext_proc::ACTION_REPLACE_BODY => {
                if let Some(ref new_payload) = response.replaced_body {
                    let framed = ext_proc::encode_grpc_frame(new_payload);
                    let original_len = if is_request {
                        self.request_body.len()
                    } else {
                        self.response_body.len()
                    };
                    if is_request {
                        self.set_http_request_body(0, original_len, &framed);
                    } else {
                        self.set_http_response_body(0, original_len, &framed);
                    }
                    log::info!("[ctx={}] ext_proc: replaced body", self.context_id);
                }
            }
            ext_proc::ACTION_REJECT => {
                log::info!("[ctx={}] ext_proc: rejecting request", self.context_id);
                self.send_http_response(403, vec![], Some(b"Rejected by proto filter"));
                return true;
            }
            _ => {
                log::warn!(
                    "[ctx={}] ext_proc: unknown action {}",
                    self.context_id,
                    response.action
                );
            }
        }

        // Add any headers specified by the ext_proc response
        for (key, value) in &response.headers_to_add {
            if is_request {
                self.add_http_request_header(key, value);
            } else {
                self.add_http_response_header(key, value);
            }
        }

        false
    }
}

impl Context for ProtoFilterHttp {
    fn on_grpc_call_response(&mut self, token_id: u32, status_code: u32, response_size: usize) {
        let is_request = match &self.pending_call {
            Some(PendingCall::Request(t)) if *t == token_id => true,
            Some(PendingCall::Response(t)) if *t == token_id => false,
            _ => {
                log::warn!(
                    "[ctx={}] unexpected grpc response token={}",
                    self.context_id,
                    token_id
                );
                return;
            }
        };
        self.pending_call = None;

        if status_code != 0 {
            log::error!(
                "[ctx={}] ext_proc call failed: grpc_status={}, size={}",
                self.context_id,
                status_code,
                response_size
            );
            if self.config.fail_open {
                if is_request {
                    self.resume_http_request();
                } else {
                    self.resume_http_response();
                }
            } else {
                self.send_http_response(503, vec![], Some(b"ext_proc service unavailable"));
            }
            return;
        }

        if let Some(body) = self.get_grpc_call_response_body(0, response_size) {
            let response = ext_proc::decode_process_response(&body);
            log::debug!(
                "[ctx={}] ext_proc response: action={}",
                self.context_id,
                response.action
            );
            let rejected = self.apply_response(&response, is_request);
            if rejected {
                return;
            }
        }

        if is_request {
            self.resume_http_request();
        } else {
            self.resume_http_response();
        }
    }
}

impl HttpContext for ProtoFilterHttp {
    fn on_http_request_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        let content_type = self
            .get_http_request_header("content-type")
            .unwrap_or_default();
        if !content_type.starts_with("application/grpc") {
            return Action::Continue;
        }

        let path = self.get_http_request_header(":path").unwrap_or_default();
        if let Some((service, method)) = grpc::parse_grpc_path(&path) {
            if self.should_process(service, method) {
                log::info!(
                    "[ctx={}] intercepting gRPC: {}/{}",
                    self.context_id,
                    service,
                    method
                );
                self.grpc_service = Some(service.to_string());
                self.grpc_method = Some(method.to_string());
                self.grpc_path = Some(path);
            }
        }

        Action::Continue
    }

    fn on_http_request_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        if self.grpc_path.is_none() || !self.config.process_request {
            return Action::Continue;
        }

        if let Some(chunk) = self.get_http_request_body(0, body_size) {
            self.request_body.extend_from_slice(&chunk);
        }

        if end_of_stream {
            let payload = self.request_body.clone();
            let frames = grpc::parse_grpc_frames(&payload);
            if frames.is_empty() {
                return Action::Continue;
            }

            // Send the first frame to ext_proc for processing
            let frame_payload = if self.config.max_payload_bytes > 0 {
                &frames[0][..frames[0].len().min(self.config.max_payload_bytes)]
            } else {
                frames[0]
            };

            if let Some(token) =
                self.dispatch_to_ext_proc(ext_proc::DIRECTION_REQUEST, frame_payload)
            {
                self.pending_call = Some(PendingCall::Request(token));
                return Action::Pause;
            } else if !self.config.fail_open {
                self.send_http_response(503, vec![], Some(b"ext_proc dispatch failed"));
                return Action::Pause;
            }
        }

        Action::Continue
    }

    fn on_http_response_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        Action::Continue
    }

    fn on_http_response_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        if self.grpc_path.is_none() || !self.config.process_response {
            return Action::Continue;
        }

        if let Some(chunk) = self.get_http_response_body(0, body_size) {
            self.response_body.extend_from_slice(&chunk);
        }

        if end_of_stream {
            let payload = self.response_body.clone();
            let frames = grpc::parse_grpc_frames(&payload);
            if frames.is_empty() {
                return Action::Continue;
            }

            let frame_payload = if self.config.max_payload_bytes > 0 {
                &frames[0][..frames[0].len().min(self.config.max_payload_bytes)]
            } else {
                frames[0]
            };

            if let Some(token) =
                self.dispatch_to_ext_proc(ext_proc::DIRECTION_RESPONSE, frame_payload)
            {
                self.pending_call = Some(PendingCall::Response(token));
                return Action::Pause;
            } else if !self.config.fail_open {
                self.send_http_response(503, vec![], Some(b"ext_proc dispatch failed"));
                return Action::Pause;
            }
        }

        Action::Continue
    }
}
