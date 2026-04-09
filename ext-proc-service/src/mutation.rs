use uuid::Uuid;

use crate::config::{MutationConfig, RuleConfig, ServiceConfig};
use crate::proto;

/// The mutation engine holds all rules and evaluates them against incoming messages.
pub struct MutationEngine {
    rules: Vec<proto::MutationRule>,
}

impl MutationEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Build the engine from a service configuration.
    pub fn from_config(config: &ServiceConfig) -> Self {
        let mut engine = Self::new();
        for rule_cfg in &config.rules {
            let rule = convert_rule_config(rule_cfg);
            tracing::info!(name = %rule.name, id = %rule.id, "loaded rule");
            engine.rules.push(rule);
        }
        engine.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        engine
    }

    /// Evaluate all matching rules against a request and return the response.
    pub fn evaluate(
        &self,
        request: &proto::ProcessMessageRequest,
    ) -> proto::ProcessMessageResponse {
        let mut all_mutations: Vec<proto::FieldMutation> = Vec::new();
        let mut action = proto::ResponseAction::Continue as i32;
        let mut replaced_body = Vec::new();
        let mut headers_to_add = std::collections::HashMap::new();

        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }

            if !self.rule_matches(rule, request) {
                continue;
            }

            tracing::debug!(rule_id = %rule.id, rule_name = %rule.name, "rule matched");

            match proto::ResponseAction::try_from(rule.action) {
                Ok(proto::ResponseAction::MutateFields) => {
                    let resolved = resolve_mutations(&rule.mutations);
                    all_mutations.extend(resolved);
                    if action == proto::ResponseAction::Continue as i32 {
                        action = proto::ResponseAction::MutateFields as i32;
                    }
                }
                Ok(proto::ResponseAction::ReplaceBody) => {
                    replaced_body = rule.replace_body.clone();
                    action = proto::ResponseAction::ReplaceBody as i32;
                    break; // Replace takes priority, stop evaluating
                }
                Ok(proto::ResponseAction::Reject) => {
                    action = proto::ResponseAction::Reject as i32;
                    break; // Reject takes priority, stop evaluating
                }
                _ => {}
            }
        }

        if !all_mutations.is_empty() {
            headers_to_add.insert(
                "x-proto-mutations-applied".to_string(),
                all_mutations.len().to_string(),
            );
        }

        proto::ProcessMessageResponse {
            action,
            mutations: all_mutations,
            replaced_body,
            headers_to_add,
        }
    }

    fn rule_matches(
        &self,
        rule: &proto::MutationRule,
        request: &proto::ProcessMessageRequest,
    ) -> bool {
        let Some(ref match_criteria) = rule.match_criteria else {
            return true; // No criteria = match all
        };

        // Check direction
        if match_criteria.direction != proto::Direction::Unspecified as i32
            && match_criteria.direction != request.direction
        {
            return false;
        }

        // Check services
        if !match_criteria.services.is_empty()
            && !match_criteria.services.iter().any(|s| {
                s == "*" || s == &request.service_name || wildcard_match(s, &request.service_name)
            })
        {
            return false;
        }

        // Check methods
        if !match_criteria.methods.is_empty()
            && !match_criteria.methods.iter().any(|m| {
                m == "*" || m == &request.method_name || wildcard_match(m, &request.method_name)
            })
        {
            return false;
        }

        // Check field conditions
        for condition in &match_criteria.field_conditions {
            if !field_condition_matches(condition, &request.decoded_fields) {
                return false;
            }
        }

        true
    }

    // ---- Rule management (used by admin service) ----

    pub fn list_rules(&self) -> Vec<proto::MutationRule> {
        self.rules.clone()
    }

    pub fn add_rule(&mut self, mut rule: proto::MutationRule) -> String {
        if rule.id.is_empty() {
            rule.id = Uuid::new_v4().to_string();
        }
        let id = rule.id.clone();
        self.rules.push(rule);
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        id
    }

    pub fn remove_rule(&mut self, rule_id: &str) -> bool {
        let len_before = self.rules.len();
        self.rules.retain(|r| r.id != rule_id);
        self.rules.len() < len_before
    }

    pub fn update_rule(&mut self, rule: proto::MutationRule) -> bool {
        if let Some(existing) = self.rules.iter_mut().find(|r| r.id == rule.id) {
            *existing = rule;
            self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
            true
        } else {
            false
        }
    }
}

/// Resolve dynamic values in mutations (e.g., UUID, timestamp).
fn resolve_mutations(mutations: &[proto::FieldMutation]) -> Vec<proto::FieldMutation> {
    mutations
        .iter()
        .map(|m| {
            let mut resolved = m.clone();
            // Check if the value looks like a dynamic placeholder
            if let Ok(s) = std::str::from_utf8(&m.value) {
                match s {
                    "$uuid" => {
                        resolved.value = Uuid::new_v4().to_string().into_bytes();
                    }
                    "$timestamp" => {
                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        if m.field_type == proto::FieldType::Varint as i32 {
                            resolved.value = encode_varint(ts);
                        } else {
                            resolved.value = ts.to_string().into_bytes();
                        }
                    }
                    _ => {}
                }
            }
            resolved
        })
        .collect()
}

/// Simple wildcard matching (supports trailing '*' only, e.g. "mypackage.*").
fn wildcard_match(pattern: &str, value: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        value.starts_with(prefix)
    } else {
        pattern == value
    }
}

fn field_condition_matches(
    condition: &proto::FieldCondition,
    fields: &[proto::ProtoField],
) -> bool {
    let matching_field = fields
        .iter()
        .find(|f| f.field_number == condition.field_number);

    match proto::ConditionOp::try_from(condition.operator) {
        Ok(proto::ConditionOp::Exists) => matching_field.is_some(),
        Ok(proto::ConditionOp::NotExists) => matching_field.is_none(),
        Ok(proto::ConditionOp::Equals) => {
            matching_field.is_some_and(|f| f.raw_value == condition.value)
        }
        Ok(proto::ConditionOp::NotEquals) => {
            matching_field.is_none_or(|f| f.raw_value != condition.value)
        }
        Ok(proto::ConditionOp::Contains) => matching_field.is_some_and(|f| {
            if let (Ok(field_str), Ok(cond_str)) = (
                std::str::from_utf8(&f.raw_value),
                std::str::from_utf8(&condition.value),
            ) {
                field_str.contains(cond_str)
            } else {
                // Fall back to byte subsequence search
                f.raw_value
                    .windows(condition.value.len())
                    .any(|w| w == condition.value.as_slice())
            }
        }),
        _ => true,
    }
}

/// Encode a u64 as a protobuf varint.
fn encode_varint(mut value: u64) -> Vec<u8> {
    let mut buf = Vec::new();
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
    buf
}

/// Convert a YAML rule config into a proto MutationRule.
fn convert_rule_config(cfg: &RuleConfig) -> proto::MutationRule {
    let direction = match cfg.match_criteria.direction.as_str() {
        "request" => proto::Direction::Request as i32,
        "response" => proto::Direction::Response as i32,
        _ => proto::Direction::Unspecified as i32,
    };

    let action = match cfg.action.as_str() {
        "mutate_fields" => proto::ResponseAction::MutateFields as i32,
        "replace_body" => proto::ResponseAction::ReplaceBody as i32,
        "reject" => proto::ResponseAction::Reject as i32,
        _ => proto::ResponseAction::Continue as i32,
    };

    let mutations: Vec<proto::FieldMutation> =
        cfg.mutations.iter().map(convert_mutation_config).collect();

    let field_conditions: Vec<proto::FieldCondition> = cfg
        .match_criteria
        .field_conditions
        .iter()
        .map(|fc| {
            let operator = match fc.operator.as_str() {
                "exists" => proto::ConditionOp::Exists as i32,
                "not_exists" => proto::ConditionOp::NotExists as i32,
                "equals" => proto::ConditionOp::Equals as i32,
                "not_equals" => proto::ConditionOp::NotEquals as i32,
                "contains" => proto::ConditionOp::Contains as i32,
                _ => proto::ConditionOp::Exists as i32,
            };
            proto::FieldCondition {
                field_number: fc.field_number,
                operator,
                value: fc.value.as_deref().unwrap_or("").as_bytes().to_vec(),
            }
        })
        .collect();

    let replace_body = cfg
        .replace_body
        .as_ref()
        .map(|b| b.as_bytes().to_vec())
        .unwrap_or_default();

    proto::MutationRule {
        id: Uuid::new_v4().to_string(),
        name: cfg.name.clone(),
        match_criteria: Some(proto::RuleMatch {
            services: cfg.match_criteria.services.clone(),
            methods: cfg.match_criteria.methods.clone(),
            direction,
            field_conditions,
        }),
        action,
        mutations,
        replace_body,
        priority: cfg.priority,
        enabled: cfg.enabled,
    }
}

fn convert_mutation_config(cfg: &MutationConfig) -> proto::FieldMutation {
    let operation = match cfg.operation.as_str() {
        "set" => proto::MutationOp::Set as i32,
        "add" => proto::MutationOp::Add as i32,
        "remove" => proto::MutationOp::Remove as i32,
        _ => proto::MutationOp::Set as i32,
    };

    let field_type = match cfg.field_type.as_str() {
        "varint" => proto::FieldType::Varint as i32,
        "fixed64" => proto::FieldType::Fixed64 as i32,
        "fixed32" => proto::FieldType::Fixed32 as i32,
        _ => proto::FieldType::LengthDelimited as i32,
    };

    // Resolve the value based on value_source and field_type
    let value = match cfg.value_source.as_str() {
        "uuid" => "$uuid".as_bytes().to_vec(),
        "timestamp" => "$timestamp".as_bytes().to_vec(),
        _ => {
            // Static value — encode based on field_type
            let raw = cfg.value.as_deref().unwrap_or("");
            match cfg.field_type.as_str() {
                "varint" => {
                    let n: u64 = raw.parse().unwrap_or(0);
                    encode_varint(n)
                }
                "fixed32" => {
                    let n: u32 = raw.parse().unwrap_or(0);
                    n.to_le_bytes().to_vec()
                }
                "fixed64" => {
                    let n: u64 = raw.parse().unwrap_or(0);
                    n.to_le_bytes().to_vec()
                }
                _ => raw.as_bytes().to_vec(),
            }
        }
    };

    proto::FieldMutation {
        operation,
        field_number: cfg.field_number,
        field_type,
        value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(
        service: &str,
        method: &str,
        direction: i32,
        fields: Vec<proto::ProtoField>,
    ) -> proto::ProcessMessageRequest {
        proto::ProcessMessageRequest {
            service_name: service.to_string(),
            method_name: method.to_string(),
            direction,
            raw_body: Vec::new(),
            decoded_fields: fields,
        }
    }

    fn make_rule(
        name: &str,
        services: Vec<&str>,
        methods: Vec<&str>,
        direction: i32,
        action: i32,
        mutations: Vec<proto::FieldMutation>,
    ) -> proto::MutationRule {
        proto::MutationRule {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            match_criteria: Some(proto::RuleMatch {
                services: services.into_iter().map(String::from).collect(),
                methods: methods.into_iter().map(String::from).collect(),
                direction,
                field_conditions: Vec::new(),
            }),
            action,
            mutations,
            replace_body: Vec::new(),
            priority: 0,
            enabled: true,
        }
    }

    #[test]
    fn test_no_rules_returns_continue() {
        let engine = MutationEngine::new();
        let req = make_request("svc", "method", 1, Vec::new());
        let resp = engine.evaluate(&req);
        assert_eq!(resp.action, proto::ResponseAction::Continue as i32);
    }

    #[test]
    fn test_matching_rule_applies_mutations() {
        let mut engine = MutationEngine::new();
        engine.add_rule(make_rule(
            "test",
            vec!["*"],
            vec!["*"],
            proto::Direction::Unspecified as i32,
            proto::ResponseAction::MutateFields as i32,
            vec![proto::FieldMutation {
                operation: proto::MutationOp::Add as i32,
                field_number: 100,
                field_type: proto::FieldType::LengthDelimited as i32,
                value: b"injected".to_vec(),
            }],
        ));

        let req = make_request(
            "mypackage.Service",
            "GetUser",
            proto::Direction::Request as i32,
            Vec::new(),
        );
        let resp = engine.evaluate(&req);
        assert_eq!(resp.action, proto::ResponseAction::MutateFields as i32);
        assert_eq!(resp.mutations.len(), 1);
        assert_eq!(resp.mutations[0].field_number, 100);
    }

    #[test]
    fn test_direction_filter() {
        let mut engine = MutationEngine::new();
        engine.add_rule(make_rule(
            "request-only",
            vec!["*"],
            vec!["*"],
            proto::Direction::Request as i32,
            proto::ResponseAction::MutateFields as i32,
            vec![proto::FieldMutation {
                operation: proto::MutationOp::Add as i32,
                field_number: 50,
                field_type: proto::FieldType::Varint as i32,
                value: encode_varint(1),
            }],
        ));

        // Should match request
        let req = make_request("svc", "m", proto::Direction::Request as i32, Vec::new());
        let resp = engine.evaluate(&req);
        assert_eq!(resp.mutations.len(), 1);

        // Should not match response
        let req = make_request("svc", "m", proto::Direction::Response as i32, Vec::new());
        let resp = engine.evaluate(&req);
        assert!(resp.mutations.is_empty());
    }

    #[test]
    fn test_service_wildcard_match() {
        let mut engine = MutationEngine::new();
        engine.add_rule(make_rule(
            "mypackage-only",
            vec!["mypackage.*"],
            vec!["*"],
            proto::Direction::Unspecified as i32,
            proto::ResponseAction::MutateFields as i32,
            vec![proto::FieldMutation {
                operation: proto::MutationOp::Add as i32,
                field_number: 10,
                field_type: proto::FieldType::Varint as i32,
                value: encode_varint(1),
            }],
        ));

        let req = make_request("mypackage.UserService", "Get", 1, Vec::new());
        let resp = engine.evaluate(&req);
        assert_eq!(resp.mutations.len(), 1);

        let req = make_request("other.Service", "Get", 1, Vec::new());
        let resp = engine.evaluate(&req);
        assert!(resp.mutations.is_empty());
    }

    #[test]
    fn test_reject_stops_evaluation() {
        let mut engine = MutationEngine::new();
        engine.add_rule(make_rule(
            "reject",
            vec!["*"],
            vec!["*"],
            0,
            proto::ResponseAction::Reject as i32,
            Vec::new(),
        ));
        engine.add_rule(make_rule(
            "add-field",
            vec!["*"],
            vec!["*"],
            0,
            proto::ResponseAction::MutateFields as i32,
            vec![proto::FieldMutation {
                operation: proto::MutationOp::Add as i32,
                field_number: 1,
                field_type: 0,
                value: encode_varint(1),
            }],
        ));

        let req = make_request("svc", "m", 1, Vec::new());
        let resp = engine.evaluate(&req);
        assert_eq!(resp.action, proto::ResponseAction::Reject as i32);
        assert!(resp.mutations.is_empty());
    }

    #[test]
    fn test_add_and_remove_rule() {
        let mut engine = MutationEngine::new();
        let id = engine.add_rule(make_rule(
            "temp",
            vec!["*"],
            vec!["*"],
            0,
            proto::ResponseAction::Reject as i32,
            Vec::new(),
        ));
        assert_eq!(engine.list_rules().len(), 1);

        let removed = engine.remove_rule(&id);
        assert!(removed);
        assert!(engine.list_rules().is_empty());
    }

    #[test]
    fn test_field_condition_exists() {
        let mut engine = MutationEngine::new();
        let mut rule = make_rule(
            "if-field-1-exists",
            vec!["*"],
            vec!["*"],
            0,
            proto::ResponseAction::MutateFields as i32,
            vec![proto::FieldMutation {
                operation: proto::MutationOp::Add as i32,
                field_number: 99,
                field_type: proto::FieldType::LengthDelimited as i32,
                value: b"matched".to_vec(),
            }],
        );
        if let Some(ref mut mc) = rule.match_criteria {
            mc.field_conditions.push(proto::FieldCondition {
                field_number: 1,
                operator: proto::ConditionOp::Exists as i32,
                value: Vec::new(),
            });
        }
        engine.add_rule(rule);

        // With field 1 present
        let req = make_request(
            "svc",
            "m",
            1,
            vec![proto::ProtoField {
                field_number: 1,
                field_type: 0,
                raw_value: encode_varint(42),
                display_value: "42".to_string(),
            }],
        );
        let resp = engine.evaluate(&req);
        assert_eq!(resp.mutations.len(), 1);

        // Without field 1
        let req = make_request("svc", "m", 1, Vec::new());
        let resp = engine.evaluate(&req);
        assert!(resp.mutations.is_empty());
    }
}
