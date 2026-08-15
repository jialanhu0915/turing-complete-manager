//! Snappy decompression/compression used by circuit containers.
//!
//! Thin wrapper around the `snap` crate so the rest of the module sees
//! `Result<Vec<u8>, String>` errors instead of `snap::Error`.

/// Safety cap on decompressed size. The Python reference uses 256 MB; we
/// double that to allow for unusually large campaign files.
pub const MAX_DECOMPRESSED_SIZE: u32 = 512 * 1024 * 1024;

/// Decompress a standard Snappy frame. Returns the body bytes.
pub fn decompress(input: &[u8]) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; MAX_DECOMPRESSED_SIZE as usize];
    let n = snap::raw::Decoder::new()
        .decompress(input, &mut buf)
        .map_err(|e| format!("CIRCUIT_SNAPPY|{e}"))?;
    buf.truncate(n);
    Ok(buf)
}

/// Compress a body into a standard Snappy frame.
pub fn compress(input: &[u8]) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; snap::raw::max_compress_len(input.len())];
    let n = snap::raw::Encoder::new()
        .compress(input, &mut buf)
        .map_err(|e| format!("CIRCUIT_SNAPPY_COMPRESS|{e}"))?;
    buf.truncate(n);
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_literal_payload() {
        let original = b"hello world hello world hello world".to_vec();
        let compressed = compress(&original).unwrap();
        // Compression should produce SOMETHING, and decompression recovers it.
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn round_trip_copy_block_payload() {
        // Long repeated string exercises Snappy's COPY_2/COPY_4 opcodes.
        let original: Vec<u8> = (0..1024).map(|i| b'a' + (i % 26) as u8).collect();
        let compressed = compress(&original).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn decompress_truncated_input_errors() {
        let err = decompress(&[0xff, 0xff, 0xff]).unwrap_err();
        assert!(err.starts_with("CIRCUIT_SNAPPY|"), "got: {err}");
    }

    #[test]
    fn decompress_empty_input_errors() {
        let err = decompress(&[]).unwrap_err();
        assert!(err.starts_with("CIRCUIT_SNAPPY|"), "got: {err}");
    }
}