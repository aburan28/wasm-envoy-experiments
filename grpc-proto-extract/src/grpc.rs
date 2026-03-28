/// Parse a gRPC path like "/package.ServiceName/MethodName" into (service, method).
pub fn parse_grpc_path(path: &str) -> Option<(&str, &str)> {
    let path = path.strip_prefix('/')?;
    let (service, method) = path.split_once('/')?;
    if service.is_empty() || method.is_empty() {
        return None;
    }
    Some((service, method))
}

/// Parse gRPC length-prefixed frames from a body buffer.
///
/// Each frame is:
///   - 1 byte: compressed flag (0 = uncompressed, 1 = compressed)
///   - 4 bytes: message length (big-endian u32)
///   - N bytes: protobuf payload
///
/// Returns a Vec of the raw protobuf payloads (skipping compressed frames).
pub fn parse_grpc_frames(data: &[u8]) -> Vec<&[u8]> {
    let mut frames = Vec::new();
    let mut offset = 0;

    while offset + 5 <= data.len() {
        let compressed = data[offset];
        let length = u32::from_be_bytes([
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
        ]) as usize;

        offset += 5;

        if offset + length > data.len() {
            log::warn!(
                "grpc frame truncated: expected {} bytes, have {}",
                length,
                data.len() - offset
            );
            break;
        }

        if compressed == 0 {
            frames.push(&data[offset..offset + length]);
        } else {
            log::debug!("skipping compressed grpc frame ({} bytes)", length);
        }

        offset += length;
    }

    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_grpc_path() {
        assert_eq!(
            parse_grpc_path("/mypackage.MyService/MyMethod"),
            Some(("mypackage.MyService", "MyMethod"))
        );
        assert_eq!(parse_grpc_path("/"), None);
        assert_eq!(parse_grpc_path(""), None);
        assert_eq!(parse_grpc_path("/service/"), None);
    }

    #[test]
    fn test_parse_grpc_frames_single() {
        // uncompressed, 3-byte payload
        let data = [0u8, 0, 0, 0, 3, 0x08, 0x96, 0x01];
        let frames = parse_grpc_frames(&data);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], &[0x08, 0x96, 0x01]);
    }

    #[test]
    fn test_parse_grpc_frames_multiple() {
        let mut data = Vec::new();
        // frame 1: 2 bytes
        data.extend_from_slice(&[0, 0, 0, 0, 2, 0x10, 0x01]);
        // frame 2: 1 byte
        data.extend_from_slice(&[0, 0, 0, 0, 1, 0x18]);
        let frames = parse_grpc_frames(&data);
        assert_eq!(frames.len(), 2);
    }

    #[test]
    fn test_parse_grpc_frames_truncated() {
        // header says 10 bytes but only 3 available
        let data = [0u8, 0, 0, 0, 10, 1, 2, 3];
        let frames = parse_grpc_frames(&data);
        assert_eq!(frames.len(), 0);
    }
}
