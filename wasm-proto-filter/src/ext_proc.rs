use crate::proto_decode::{self, RawField, RawValue};
use crate::proto_encode;

// --- Direction enum values (matches service.proto) ---
pub const DIRECTION_REQUEST: u64 = 1;
pub const DIRECTION_RESPONSE: u64 = 2;

// --- ResponseAction enum values (matches service.proto) ---
pub const ACTION_CONTINUE: u64 = 0;
pub const ACTION_MUTATE_FIELDS: u64 = 1;
pub const ACTION_REPLACE_BODY: u64 = 2;
pub const ACTION_REJECT: u64 = 3;

// --- MutationOp enum values (matches service.proto) ---
pub const MUTATION_OP_SET: u64 = 0;
pub const MUTATION_OP_ADD: u64 = 1;
pub const MUTATION_OP_REMOVE: u64 = 2;

// ---------------------------------------------------------------------------
// ProcessMessageRequest encoding (hand-rolled protobuf, no codegen)
//
// Proto field layout:
//   1: service_name  (string)
//   2: method_name   (string)
//   3: direction     (enum / varint)
//   4: raw_body      (bytes)
//   5: decoded_fields (repeated ProtoField message)
// ---------------------------------------------------------------------------

/// Encode a ProcessMessageRequest as raw protobuf bytes.
pub fn encode_process_request(
    service_name: &str,
    method_name: &str,
    direction: u64,
    raw_body: &[u8],
    decoded_fields: &[RawField],
) -> Vec<u8> {
    let mut buf = Vec::new();

    if !service_name.is_empty() {
        buf.extend(proto_encode::encode_bytes_field(1, service_name.as_bytes()));
    }
    if !method_name.is_empty() {
        buf.extend(proto_encode::encode_bytes_field(2, method_name.as_bytes()));
    }
    if direction != 0 {
        buf.extend(proto_encode::encode_varint_field(3, direction));
    }
    if !raw_body.is_empty() {
        buf.extend(proto_encode::encode_bytes_field(4, raw_body));
    }
    for field in decoded_fields {
        let sub_msg = encode_proto_field(field);
        buf.extend(proto_encode::encode_bytes_field(5, &sub_msg));
    }

    buf
}

/// Encode a ProtoField sub-message.
///
/// Proto field layout:
///   1: field_number  (uint32)
///   2: field_type    (enum / varint)
///   3: raw_value     (bytes)
///   4: display_value (string)
fn encode_proto_field(field: &RawField) -> Vec<u8> {
    let mut buf = Vec::new();

    buf.extend(proto_encode::encode_varint_field(
        1,
        field.field_number as u64,
    ));

    let wire_type = match &field.value {
        RawValue::Varint(_) => 0u64,
        RawValue::Fixed64(_) => 1,
        RawValue::LengthDelimited(_) => 2,
        RawValue::Fixed32(_) => 5,
    };
    buf.extend(proto_encode::encode_varint_field(2, wire_type));

    let raw = raw_value_bytes(&field.value);
    buf.extend(proto_encode::encode_bytes_field(3, &raw));

    let display = format!("{:?}", field.value);
    buf.extend(proto_encode::encode_bytes_field(4, display.as_bytes()));

    buf
}

/// Extract raw bytes for a RawValue.
fn raw_value_bytes(value: &RawValue) -> Vec<u8> {
    match value {
        RawValue::Varint(v) => proto_encode::encode_varint(*v),
        RawValue::Fixed64(v) => v.to_le_bytes().to_vec(),
        RawValue::LengthDelimited(bytes) => bytes.clone(),
        RawValue::Fixed32(v) => v.to_le_bytes().to_vec(),
    }
}

// ---------------------------------------------------------------------------
// ProcessMessageResponse decoding
//
// Proto field layout:
//   1: action         (enum / varint)
//   2: mutations      (repeated FieldMutation message)
//   3: replaced_body  (bytes)
//   4: headers_to_add (map<string,string> = repeated message{1:key, 2:value})
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ProcessMessageResponse {
    pub action: u64,
    pub mutations: Vec<FieldMutation>,
    pub replaced_body: Option<Vec<u8>>,
    pub headers_to_add: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct FieldMutation {
    pub operation: u64,
    pub field_number: u32,
    pub field_type: u64,
    pub value: Vec<u8>,
}

/// Decode a ProcessMessageResponse from raw protobuf bytes.
pub fn decode_process_response(data: &[u8]) -> ProcessMessageResponse {
    let fields = proto_decode::decode_raw(data);
    let mut response = ProcessMessageResponse {
        action: ACTION_CONTINUE,
        mutations: Vec::new(),
        replaced_body: None,
        headers_to_add: Vec::new(),
    };

    for field in &fields {
        match field.field_number {
            1 => {
                if let RawValue::Varint(v) = &field.value {
                    response.action = *v;
                }
            }
            2 => {
                if let RawValue::LengthDelimited(bytes) = &field.value {
                    if let Some(mutation) = decode_field_mutation(bytes) {
                        response.mutations.push(mutation);
                    }
                }
            }
            3 => {
                if let RawValue::LengthDelimited(bytes) = &field.value {
                    response.replaced_body = Some(bytes.clone());
                }
            }
            4 => {
                if let RawValue::LengthDelimited(bytes) = &field.value {
                    if let Some((k, v)) = decode_map_entry(bytes) {
                        response.headers_to_add.push((k, v));
                    }
                }
            }
            _ => {}
        }
    }

    response
}

/// Decode a FieldMutation sub-message.
///
///   1: operation    (enum / varint)
///   2: field_number (uint32)
///   3: field_type   (enum / varint)
///   4: value        (bytes)
fn decode_field_mutation(data: &[u8]) -> Option<FieldMutation> {
    let fields = proto_decode::decode_raw(data);
    let mut mutation = FieldMutation {
        operation: 0,
        field_number: 0,
        field_type: 0,
        value: Vec::new(),
    };

    for field in &fields {
        match field.field_number {
            1 => {
                if let RawValue::Varint(v) = &field.value {
                    mutation.operation = *v;
                }
            }
            2 => {
                if let RawValue::Varint(v) = &field.value {
                    mutation.field_number = *v as u32;
                }
            }
            3 => {
                if let RawValue::Varint(v) = &field.value {
                    mutation.field_type = *v;
                }
            }
            4 => {
                if let RawValue::LengthDelimited(bytes) = &field.value {
                    mutation.value = bytes.clone();
                }
            }
            _ => {}
        }
    }

    if mutation.field_number > 0 || mutation.operation == MUTATION_OP_REMOVE {
        Some(mutation)
    } else {
        None
    }
}

/// Decode a map<string,string> entry (sub-message with fields 1 and 2).
fn decode_map_entry(data: &[u8]) -> Option<(String, String)> {
    let fields = proto_decode::decode_raw(data);
    let mut key = None;
    let mut value = None;

    for field in &fields {
        match field.field_number {
            1 => {
                if let RawValue::LengthDelimited(bytes) = &field.value {
                    key = String::from_utf8(bytes.clone()).ok();
                }
            }
            2 => {
                if let RawValue::LengthDelimited(bytes) = &field.value {
                    value = String::from_utf8(bytes.clone()).ok();
                }
            }
            _ => {}
        }
    }

    match (key, value) {
        (Some(k), Some(v)) => Some((k, v)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Mutation application
// ---------------------------------------------------------------------------

/// Apply a list of mutations to a raw protobuf payload, returning the new payload.
pub fn apply_mutations(original: &[u8], mutations: &[FieldMutation]) -> Vec<u8> {
    let mut fields = proto_decode::decode_raw(original);

    for mutation in mutations {
        match mutation.operation {
            MUTATION_OP_SET => {
                let new_value = bytes_to_raw_value(mutation.field_type, &mutation.value);
                let mut found = false;
                for field in &mut fields {
                    if field.field_number == mutation.field_number {
                        field.value = new_value.clone();
                        found = true;
                        break;
                    }
                }
                if !found {
                    fields.push(RawField {
                        field_number: mutation.field_number,
                        value: new_value,
                    });
                }
            }
            MUTATION_OP_ADD => {
                fields.push(RawField {
                    field_number: mutation.field_number,
                    value: bytes_to_raw_value(mutation.field_type, &mutation.value),
                });
            }
            MUTATION_OP_REMOVE => {
                fields.retain(|f| f.field_number != mutation.field_number);
            }
            _ => {
                log::warn!("unknown mutation op: {}", mutation.operation);
            }
        }
    }

    encode_fields(&fields)
}

/// Convert raw bytes to a RawValue based on the field type enum.
fn bytes_to_raw_value(field_type: u64, data: &[u8]) -> RawValue {
    match field_type {
        0 => {
            // Varint
            proto_decode::read_varint(data)
                .map(|(v, _)| RawValue::Varint(v))
                .unwrap_or(RawValue::Varint(0))
        }
        1 => {
            // Fixed64
            if data.len() >= 8 {
                RawValue::Fixed64(u64::from_le_bytes(data[..8].try_into().unwrap()))
            } else {
                RawValue::Fixed64(0)
            }
        }
        2 => RawValue::LengthDelimited(data.to_vec()),
        5 => {
            // Fixed32
            if data.len() >= 4 {
                RawValue::Fixed32(u32::from_le_bytes(data[..4].try_into().unwrap()))
            } else {
                RawValue::Fixed32(0)
            }
        }
        _ => RawValue::LengthDelimited(data.to_vec()),
    }
}

/// Re-encode a list of RawFields into protobuf bytes.
fn encode_fields(fields: &[RawField]) -> Vec<u8> {
    let mut buf = Vec::new();
    for field in fields {
        match &field.value {
            RawValue::Varint(v) => {
                buf.extend(proto_encode::encode_varint_field(field.field_number, *v));
            }
            RawValue::Fixed64(v) => {
                buf.extend(proto_encode::encode_fixed64_field(field.field_number, *v));
            }
            RawValue::LengthDelimited(data) => {
                buf.extend(proto_encode::encode_bytes_field(field.field_number, data));
            }
            RawValue::Fixed32(v) => {
                buf.extend(proto_encode::encode_fixed32_field(field.field_number, *v));
            }
        }
    }
    buf
}

/// Wrap a protobuf payload in a gRPC frame (uncompressed).
pub fn encode_grpc_frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(5 + payload.len());
    frame.push(0); // compressed flag = 0
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_process_request_roundtrip() {
        let request = encode_process_request(
            "my.Service",
            "MyMethod",
            DIRECTION_REQUEST,
            &[0x08, 0x96, 0x01],
            &[RawField {
                field_number: 1,
                value: RawValue::Varint(150),
            }],
        );
        let fields = proto_decode::decode_raw(&request);
        assert!(fields.len() >= 4);
        // field 1 = service_name
        assert_eq!(fields[0].field_number, 1);
        if let RawValue::LengthDelimited(v) = &fields[0].value {
            assert_eq!(v, b"my.Service");
        } else {
            panic!("expected string for service_name");
        }
    }

    #[test]
    fn test_decode_process_response_continue() {
        // action = 0 (CONTINUE) encoded as varint field 1
        let data = proto_encode::encode_varint_field(1, ACTION_CONTINUE);
        let resp = decode_process_response(&data);
        assert_eq!(resp.action, ACTION_CONTINUE);
        assert!(resp.mutations.is_empty());
    }

    #[test]
    fn test_decode_process_response_with_mutations() {
        let mut data = Vec::new();
        // action = MUTATE_FIELDS
        data.extend(proto_encode::encode_varint_field(1, ACTION_MUTATE_FIELDS));
        // mutation: SET field 5, varint, value encoded as varint(42)
        let mut mutation_msg = Vec::new();
        mutation_msg.extend(proto_encode::encode_varint_field(1, MUTATION_OP_SET));
        mutation_msg.extend(proto_encode::encode_varint_field(2, 5)); // field_number
        mutation_msg.extend(proto_encode::encode_varint_field(3, 0)); // field_type = VARINT
        mutation_msg.extend(proto_encode::encode_bytes_field(
            4,
            &proto_encode::encode_varint(42),
        ));
        data.extend(proto_encode::encode_bytes_field(2, &mutation_msg));

        let resp = decode_process_response(&data);
        assert_eq!(resp.action, ACTION_MUTATE_FIELDS);
        assert_eq!(resp.mutations.len(), 1);
        assert_eq!(resp.mutations[0].field_number, 5);
        assert_eq!(resp.mutations[0].operation, MUTATION_OP_SET);
    }

    #[test]
    fn test_apply_mutations_set_existing() {
        let original = proto_encode::encode_varint_field(1, 100);
        let mutations = vec![FieldMutation {
            operation: MUTATION_OP_SET,
            field_number: 1,
            field_type: 0,
            value: proto_encode::encode_varint(200),
        }];
        let result = apply_mutations(&original, &mutations);
        let fields = proto_decode::decode_raw(&result);
        assert_eq!(fields.len(), 1);
        match &fields[0].value {
            RawValue::Varint(v) => assert_eq!(*v, 200),
            _ => panic!("expected varint"),
        }
    }

    #[test]
    fn test_apply_mutations_set_new_field() {
        let original = proto_encode::encode_varint_field(1, 100);
        let mutations = vec![FieldMutation {
            operation: MUTATION_OP_SET,
            field_number: 2,
            field_type: 2,
            value: b"hello".to_vec(),
        }];
        let result = apply_mutations(&original, &mutations);
        let fields = proto_decode::decode_raw(&result);
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[1].field_number, 2);
    }

    #[test]
    fn test_apply_mutations_add() {
        let original = proto_encode::encode_varint_field(1, 100);
        let mutations = vec![FieldMutation {
            operation: MUTATION_OP_ADD,
            field_number: 2,
            field_type: 2,
            value: b"hello".to_vec(),
        }];
        let result = apply_mutations(&original, &mutations);
        let fields = proto_decode::decode_raw(&result);
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[1].field_number, 2);
        if let RawValue::LengthDelimited(v) = &fields[1].value {
            assert_eq!(v, b"hello");
        } else {
            panic!("expected bytes");
        }
    }

    #[test]
    fn test_apply_mutations_remove() {
        let mut original = proto_encode::encode_varint_field(1, 100);
        original.extend(proto_encode::encode_bytes_field(2, b"hello"));
        let mutations = vec![FieldMutation {
            operation: MUTATION_OP_REMOVE,
            field_number: 1,
            field_type: 0,
            value: Vec::new(),
        }];
        let result = apply_mutations(&original, &mutations);
        let fields = proto_decode::decode_raw(&result);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_number, 2);
    }

    #[test]
    fn test_encode_grpc_frame() {
        let payload = vec![0x08, 0x96, 0x01];
        let frame = encode_grpc_frame(&payload);
        assert_eq!(frame[0], 0); // not compressed
        assert_eq!(
            u32::from_be_bytes([frame[1], frame[2], frame[3], frame[4]]),
            3
        );
        assert_eq!(&frame[5..], &payload);
    }

    #[test]
    fn test_apply_multiple_mutations() {
        let mut original = Vec::new();
        original.extend(proto_encode::encode_varint_field(1, 10));
        original.extend(proto_encode::encode_bytes_field(2, b"old"));
        original.extend(proto_encode::encode_varint_field(3, 99));

        let mutations = vec![
            FieldMutation {
                operation: MUTATION_OP_SET,
                field_number: 2,
                field_type: 2,
                value: b"new".to_vec(),
            },
            FieldMutation {
                operation: MUTATION_OP_REMOVE,
                field_number: 3,
                field_type: 0,
                value: Vec::new(),
            },
            FieldMutation {
                operation: MUTATION_OP_ADD,
                field_number: 4,
                field_type: 0,
                value: proto_encode::encode_varint(777),
            },
        ];
        let result = apply_mutations(&original, &mutations);
        let fields = proto_decode::decode_raw(&result);
        assert_eq!(fields.len(), 3); // field 1 (unchanged), field 2 (set), field 4 (added)
        assert_eq!(fields[0].field_number, 1);
        assert_eq!(fields[1].field_number, 2);
        if let RawValue::LengthDelimited(v) = &fields[1].value {
            assert_eq!(v, b"new");
        }
        assert_eq!(fields[2].field_number, 4);
    }
}
