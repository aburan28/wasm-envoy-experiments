/// Encode a varint value.
pub fn encode_varint(mut value: u64) -> Vec<u8> {
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

/// Encode a protobuf tag (field_number << 3 | wire_type).
pub fn encode_tag(field_number: u32, wire_type: u8) -> Vec<u8> {
    encode_varint(((field_number as u64) << 3) | (wire_type as u64))
}

/// Encode a varint field (wire type 0).
pub fn encode_varint_field(field_number: u32, value: u64) -> Vec<u8> {
    let mut buf = encode_tag(field_number, 0);
    buf.extend(encode_varint(value));
    buf
}

/// Encode a length-delimited field (wire type 2) for strings, bytes, embedded messages.
pub fn encode_bytes_field(field_number: u32, data: &[u8]) -> Vec<u8> {
    let mut buf = encode_tag(field_number, 2);
    buf.extend(encode_varint(data.len() as u64));
    buf.extend_from_slice(data);
    buf
}

/// Encode a fixed32 field (wire type 5).
pub fn encode_fixed32_field(field_number: u32, value: u32) -> Vec<u8> {
    let mut buf = encode_tag(field_number, 5);
    buf.extend_from_slice(&value.to_le_bytes());
    buf
}

/// Encode a fixed64 field (wire type 1).
pub fn encode_fixed64_field(field_number: u32, value: u64) -> Vec<u8> {
    let mut buf = encode_tag(field_number, 1);
    buf.extend_from_slice(&value.to_le_bytes());
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_varint_small() {
        assert_eq!(encode_varint(0), vec![0x00]);
        assert_eq!(encode_varint(1), vec![0x01]);
        assert_eq!(encode_varint(127), vec![0x7F]);
    }

    #[test]
    fn test_encode_varint_150() {
        assert_eq!(encode_varint(150), vec![0x96, 0x01]);
    }

    #[test]
    fn test_encode_varint_field() {
        // field 1, varint, value 150 -> tag(0x08) + varint(150)
        let encoded = encode_varint_field(1, 150);
        assert_eq!(encoded, vec![0x08, 0x96, 0x01]);
    }

    #[test]
    fn test_encode_bytes_field() {
        // field 2, string "testing"
        let encoded = encode_bytes_field(2, b"testing");
        assert_eq!(
            encoded,
            vec![0x12, 0x07, 0x74, 0x65, 0x73, 0x74, 0x69, 0x6e, 0x67]
        );
    }

    #[test]
    fn test_roundtrip_varint() {
        for &val in &[0u64, 1, 127, 128, 150, 300, 16384, u64::MAX] {
            let encoded = encode_varint(val);
            let (decoded, _) = crate::proto_decode::read_varint(&encoded).unwrap();
            assert_eq!(decoded, val, "roundtrip failed for {}", val);
        }
    }

    #[test]
    fn test_encode_fixed32_field() {
        let encoded = encode_fixed32_field(3, 42);
        // tag for field 3, wire type 5 = (3 << 3) | 5 = 29 = 0x1D
        assert_eq!(encoded[0], 0x1D);
        assert_eq!(&encoded[1..], &42u32.to_le_bytes());
    }

    #[test]
    fn test_encode_fixed64_field() {
        let encoded = encode_fixed64_field(4, 12345);
        // tag for field 4, wire type 1 = (4 << 3) | 1 = 33 = 0x21
        assert_eq!(encoded[0], 0x21);
        assert_eq!(&encoded[1..], &12345u64.to_le_bytes());
    }
}
