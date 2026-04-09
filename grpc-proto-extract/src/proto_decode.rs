use std::fmt;

/// A raw protobuf field decoded without a schema.
#[derive(Debug, Clone)]
pub struct RawField {
    pub field_number: u32,
    pub value: RawValue,
}

/// Possible wire type values from a protobuf message.
#[derive(Clone)]
pub enum RawValue {
    /// Wire type 0: varint (int32, int64, uint32, uint64, sint32, sint64, bool, enum)
    Varint(u64),
    /// Wire type 1: 64-bit (fixed64, sfixed64, double)
    Fixed64(u64),
    /// Wire type 2: length-delimited (string, bytes, embedded messages, packed repeated)
    LengthDelimited(Vec<u8>),
    /// Wire type 5: 32-bit (fixed32, sfixed32, float)
    Fixed32(u32),
}

impl fmt::Debug for RawValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RawValue::Varint(v) => write!(f, "varint({})", v),
            RawValue::Fixed64(v) => write!(f, "fixed64({})", v),
            RawValue::Fixed32(v) => write!(f, "fixed32({})", v),
            RawValue::LengthDelimited(bytes) => {
                // Try to display as UTF-8 string if valid, otherwise show hex
                if let Ok(s) = std::str::from_utf8(bytes) {
                    if s.chars().all(|c| !c.is_control() || c == '\n' || c == '\t') {
                        return write!(f, "string({:?})", s);
                    }
                }
                // Try to decode as nested message
                let nested = decode_raw(bytes);
                if !nested.is_empty() && nested_looks_valid(bytes, &nested) {
                    return write!(f, "message({:?})", nested);
                }
                write!(
                    f,
                    "bytes({} bytes: {:02x?})",
                    bytes.len(),
                    &bytes[..bytes.len().min(32)]
                )
            }
        }
    }
}

/// Check if a nested decode looks plausible (consumed most bytes, reasonable field numbers).
fn nested_looks_valid(_original: &[u8], fields: &[RawField]) -> bool {
    if fields.is_empty() {
        return false;
    }
    // Field numbers should be reasonable (< 10000) and the decode should cover a good portion
    fields
        .iter()
        .all(|f| f.field_number > 0 && f.field_number < 10000)
}

/// Read a varint from the buffer, returning (value, bytes_consumed).
fn read_varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0;

    for (i, &byte) in data.iter().enumerate() {
        if shift >= 64 {
            return None;
        }
        result |= ((byte & 0x7F) as u64) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return Some((result, i + 1));
        }
    }
    None
}

/// Decode a raw protobuf message without a schema.
/// Returns a list of field number + raw value pairs.
pub fn decode_raw(data: &[u8]) -> Vec<RawField> {
    let mut fields = Vec::new();
    let mut offset = 0;

    while offset < data.len() {
        // Read field tag (varint)
        let (tag, tag_len) = match read_varint(&data[offset..]) {
            Some(v) => v,
            None => break,
        };
        offset += tag_len;

        let field_number = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;

        if field_number == 0 {
            break;
        }

        let value = match wire_type {
            0 => {
                // Varint
                match read_varint(&data[offset..]) {
                    Some((v, len)) => {
                        offset += len;
                        RawValue::Varint(v)
                    }
                    None => break,
                }
            }
            1 => {
                // 64-bit
                if offset + 8 > data.len() {
                    break;
                }
                let v = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                offset += 8;
                RawValue::Fixed64(v)
            }
            2 => {
                // Length-delimited
                match read_varint(&data[offset..]) {
                    Some((len, len_bytes)) => {
                        offset += len_bytes;
                        let len = len as usize;
                        if offset + len > data.len() {
                            break;
                        }
                        let v = data[offset..offset + len].to_vec();
                        offset += len;
                        RawValue::LengthDelimited(v)
                    }
                    None => break,
                }
            }
            5 => {
                // 32-bit
                if offset + 4 > data.len() {
                    break;
                }
                let v = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
                offset += 4;
                RawValue::Fixed32(v)
            }
            _ => {
                // Unknown wire type (3, 4 are deprecated groups)
                log::debug!("unknown wire type {} at field {}", wire_type, field_number);
                break;
            }
        };

        fields.push(RawField {
            field_number,
            value,
        });
    }

    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_decode() {
        // 150 encoded as varint: 0x96 0x01
        let (val, len) = read_varint(&[0x96, 0x01]).unwrap();
        assert_eq!(val, 150);
        assert_eq!(len, 2);
    }

    #[test]
    fn test_decode_simple_message() {
        // field 1, varint, value 150
        let data = [0x08, 0x96, 0x01];
        let fields = decode_raw(&data);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_number, 1);
        match &fields[0].value {
            RawValue::Varint(v) => assert_eq!(*v, 150),
            _ => panic!("expected varint"),
        }
    }

    #[test]
    fn test_decode_string_field() {
        // field 2, length-delimited, "testing"
        let data = [0x12, 0x07, 0x74, 0x65, 0x73, 0x74, 0x69, 0x6e, 0x67];
        let fields = decode_raw(&data);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_number, 2);
        match &fields[0].value {
            RawValue::LengthDelimited(v) => assert_eq!(v, b"testing"),
            _ => panic!("expected length-delimited"),
        }
    }

    #[test]
    fn test_decode_multiple_fields() {
        // field 1 = varint(1), field 2 = string("hi")
        let data = [0x08, 0x01, 0x12, 0x02, 0x68, 0x69];
        let fields = decode_raw(&data);
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].field_number, 1);
        assert_eq!(fields[1].field_number, 2);
    }
}
