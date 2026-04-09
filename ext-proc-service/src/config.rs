use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    /// Address to listen on
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    /// Mutation rules to load at startup
    #[serde(default)]
    pub rules: Vec<RuleConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuleConfig {
    /// Human-readable rule name
    pub name: String,

    /// Matching criteria
    #[serde(default)]
    pub match_criteria: MatchConfig,

    /// Action to take: "continue", "mutate_fields", "replace_body", "reject"
    #[serde(default = "default_action")]
    pub action: String,

    /// Mutations to apply
    #[serde(default)]
    pub mutations: Vec<MutationConfig>,

    /// Replacement body (base64-encoded, for replace_body action)
    #[serde(default)]
    pub replace_body: Option<String>,

    /// Rule priority (higher = evaluated first)
    #[serde(default)]
    pub priority: i32,

    /// Whether the rule is active
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MatchConfig {
    /// gRPC service patterns (empty or ["*"] = all)
    #[serde(default)]
    pub services: Vec<String>,

    /// gRPC method patterns (empty or ["*"] = all)
    #[serde(default)]
    pub methods: Vec<String>,

    /// Direction: "request", "response", or "both" (default)
    #[serde(default = "default_direction")]
    pub direction: String,

    /// Optional field conditions
    #[serde(default)]
    pub field_conditions: Vec<FieldConditionConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldConditionConfig {
    pub field_number: u32,
    /// Operator: "exists", "not_exists", "equals", "not_equals", "contains"
    pub operator: String,
    /// Value to compare (interpreted based on context)
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MutationConfig {
    /// Operation: "set", "add", "remove"
    pub operation: String,

    /// Target protobuf field number
    pub field_number: u32,

    /// Wire type: "varint", "fixed64", "length_delimited", "fixed32"
    #[serde(default = "default_field_type")]
    pub field_type: String,

    /// Value as a string (interpreted based on field_type):
    ///   - varint: parsed as integer
    ///   - length_delimited: used as UTF-8 string bytes
    ///   - fixed32: parsed as u32
    ///   - fixed64: parsed as u64
    #[serde(default)]
    pub value: Option<String>,

    /// Value source for dynamic values: "static" (default), "uuid", "timestamp"
    #[serde(default = "default_value_source")]
    pub value_source: String,
}

fn default_listen_addr() -> String {
    "0.0.0.0:50051".to_string()
}

fn default_action() -> String {
    "mutate_fields".to_string()
}

fn default_direction() -> String {
    "both".to_string()
}

fn default_true() -> bool {
    true
}

fn default_field_type() -> String {
    "length_delimited".to_string()
}

fn default_value_source() -> String {
    "static".to_string()
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            listen_addr: default_listen_addr(),
            rules: Vec::new(),
        }
    }
}

impl ServiceConfig {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let config: ServiceConfig = serde_yaml::from_str(&contents)?;
        Ok(config)
    }
}
