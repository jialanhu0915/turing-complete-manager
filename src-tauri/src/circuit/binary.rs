//! Bounds-checked little-endian binary primitives used by circuit codecs.
//!
//! Ported from `tc-save-lab/src/tc_save_lab/binary.py`. Errors are returned as
//! `Result<T, String>` using the project's `"CODE|details"` convention.

/// Maximum number of items in a length-prefixed sequence. Defends against
/// malformed input advertising 10⁹+ items that would exhaust memory before
/// the body bytes even arrive.
pub const MAX_SEQUENCE_ITEMS: i64 = 10_000_000;

/// Maximum UTF-8 byte length for a length-prefixed string.
pub const MAX_STRING_BYTES: usize = 0xFFFF;

/// Read primitives from a borrowed byte slice. Advances an internal offset and
/// fails on truncation or out-of-range values.
pub struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    /// Construct a reader over `data`. Does not validate; callers detect
    /// truncation on first read.
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    /// Current offset, in bytes from the start of the input.
    pub const fn position(&self) -> usize {
        self.offset
    }

    /// Borrow `n` bytes from the cursor and advance. Exposed for codecs that
    /// need to consume a fixed-size opaque block (e.g. the 512-byte custom
    /// design payload).
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.offset + n > self.data.len() {
            return Err(format!(
                "CIRCUIT_TRUNCATED|need {} byte(s) at offset {}, have {}",
                n,
                self.offset,
                self.data.len().saturating_sub(self.offset)
            ));
        }
        let start = self.offset;
        self.offset += n;
        Ok(&self.data[start..start + n])
    }

    /// Single byte.
    pub fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    /// Little-endian `u16`.
    pub fn u16(&mut self) -> Result<u16, String> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// Little-endian `i16`.
    pub fn i16(&mut self) -> Result<i16, String> {
        let bytes = self.take(2)?;
        Ok(i16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// Little-endian `u32`.
    pub fn u32(&mut self) -> Result<u32, String> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Little-endian `i64`.
    pub fn i64(&mut self) -> Result<i64, String> {
        let bytes = self.take(8)?;
        Ok(i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    /// Little-endian `u64`.
    pub fn u64(&mut self) -> Result<u64, String> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    /// Read a single byte and require it to be 0 or 1.
    pub fn boolean(&mut self) -> Result<bool, String> {
        let value = self.u8()?;
        match value {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(format!(
                "CIRCUIT_BAD_BOOLEAN|{} at offset {}",
                other,
                self.offset - 1
            )),
        }
    }

    /// Length-prefixed UTF-8 string (`u16` length + payload bytes).
    pub fn string(&mut self) -> Result<String, String> {
        let len = self.u16()? as usize;
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes)
            .map(|s| s.to_owned())
            .map_err(|e| format!("CIRCUIT_UTF8|{e}"))
    }

    /// Length-prefixed opaque byte buffer (`u16` length + payload bytes).
    pub fn bytes_u16(&mut self) -> Result<Vec<u8>, String> {
        let len = self.u16()? as usize;
        Ok(self.take(len)?.to_vec())
    }

    /// Two `i16`s packed as a `(x, y)` point.
    pub fn point(&mut self) -> Result<(i16, i16), String> {
        Ok((self.i16()?, self.i16()?))
    }

    /// Read an `i64` length prefix and validate it against [`MAX_SEQUENCE_ITEMS`].
    pub fn count_i64(&mut self, label: &str) -> Result<usize, String> {
        let value = self.i64()?;
        if !(0..=MAX_SEQUENCE_ITEMS).contains(&value) {
            return Err(format!(
                "CIRCUIT_BAD_COUNT|{label}={value} exceeds {MAX_SEQUENCE_ITEMS}"
            ));
        }
        Ok(value as usize)
    }

    /// Assert the reader has consumed exactly the input.
    pub fn finish(&self) -> Result<(), String> {
        let trailing = self.data.len() - self.offset;
        if trailing != 0 {
            Err(format!("CIRCUIT_TRAILING|{trailing} unconsumed byte(s)"))
        } else {
            Ok(())
        }
    }
}

/// Write primitives to an owned `Vec<u8>`. Errors out only if a value does not
/// fit its target width (e.g. `u16` overflow).
pub struct Writer {
    pub data: Vec<u8>,
}

impl Writer {
    /// Construct an empty writer.
    pub const fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Append raw bytes. Exposed for codecs that need to emit a fixed-size
    /// opaque block (e.g. the 512-byte custom design payload).
    pub fn pack(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    /// Append a single byte.
    pub fn u8(&mut self, value: u8) {
        self.pack(&[value]);
    }

    /// Append a little-endian `u16`.
    pub fn u16(&mut self, value: u16) -> Result<(), String> {
        self.pack(&value.to_le_bytes());
        Ok(())
    }

    /// Append a little-endian `i16`.
    pub fn i16(&mut self, value: i16) -> Result<(), String> {
        self.pack(&value.to_le_bytes());
        Ok(())
    }

    /// Append a little-endian `u32`.
    pub fn u32(&mut self, value: u32) -> Result<(), String> {
        self.pack(&value.to_le_bytes());
        Ok(())
    }

    /// Append a little-endian `i64`.
    pub fn i64(&mut self, value: i64) -> Result<(), String> {
        self.pack(&value.to_le_bytes());
        Ok(())
    }

    /// Append a little-endian `u64`.
    pub fn u64(&mut self, value: u64) -> Result<(), String> {
        self.pack(&value.to_le_bytes());
        Ok(())
    }

    /// Append a `u8` 0/1 boolean.
    pub fn boolean(&mut self, value: bool) {
        self.u8(if value { 1 } else { 0 });
    }

    /// Append a `u16`-prefixed UTF-8 string.
    pub fn string(&mut self, value: &str) -> Result<(), String> {
        let bytes = value.as_bytes();
        if bytes.len() > MAX_STRING_BYTES {
            return Err(format!(
                "CIRCUIT_STRING_TOO_LONG|{} bytes exceeds {MAX_STRING_BYTES}",
                bytes.len()
            ));
        }
        self.u16(bytes.len() as u16)?;
        self.pack(bytes);
        Ok(())
    }

    /// Append a `u16`-prefixed opaque byte buffer.
    pub fn bytes_u16(&mut self, value: &[u8]) -> Result<(), String> {
        if value.len() > MAX_STRING_BYTES {
            return Err(format!(
                "CIRCUIT_BYTES_TOO_LONG|{} bytes exceeds {MAX_STRING_BYTES}",
                value.len()
            ));
        }
        self.u16(value.len() as u16)?;
        self.pack(value);
        Ok(())
    }

    /// Append two `i16`s packed as `(x, y)`.
    pub fn point(&mut self, value: (i16, i16)) -> Result<(), String> {
        self.i16(value.0)?;
        self.i16(value.1)
    }

    /// Append an `i64` length prefix.
    pub fn count_i64(&mut self, value: usize) -> Result<(), String> {
        self.i64(value as i64)
    }
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_round_trip() {
        let mut w = Writer::new();
        w.u8(0xAB);
        w.u16(0x1234).unwrap();
        w.i16(-7).unwrap();
        w.u32(0xDEAD_BEEF).unwrap();
        w.i64(-42).unwrap();
        w.u64(u64::MAX).unwrap();
        w.boolean(true);
        w.boolean(false);
        w.string("hi").unwrap();
        w.bytes_u16(&[1, 2, 3]).unwrap();
        w.point((-13, 21)).unwrap();
        w.count_i64(5).unwrap();

        let mut r = Reader::new(&w.data);
        assert_eq!(r.u8().unwrap(), 0xAB);
        assert_eq!(r.u16().unwrap(), 0x1234);
        assert_eq!(r.i16().unwrap(), -7);
        assert_eq!(r.u32().unwrap(), 0xDEAD_BEEF);
        assert_eq!(r.i64().unwrap(), -42);
        assert_eq!(r.u64().unwrap(), u64::MAX);
        assert!(r.boolean().unwrap());
        assert!(!r.boolean().unwrap());
        assert_eq!(r.string().unwrap(), "hi");
        assert_eq!(r.bytes_u16().unwrap(), vec![1, 2, 3]);
        assert_eq!(r.point().unwrap(), (-13, 21));
        assert_eq!(r.count_i64("test").unwrap(), 5);
        r.finish().unwrap();
    }

    #[test]
    fn boolean_rejects_garbage() {
        let bytes = [42u8];
        let mut r = Reader::new(&bytes);
        let err = r.boolean().unwrap_err();
        assert!(err.starts_with("CIRCUIT_BAD_BOOLEAN|"), "got: {err}");
    }

    #[test]
    fn truncation_reports_offset() {
        let bytes = [1u8, 2, 3];
        let mut r = Reader::new(&bytes);
        r.u16().unwrap(); // consume all 3 bytes via 2-byte read of 0x0201
        let err = r.u16().unwrap_err();
        assert!(err.starts_with("CIRCUIT_TRUNCATED|"), "got: {err}");
        assert!(err.contains("offset 2"), "got: {err}");
    }

    #[test]
    fn count_i64_rejects_huge() {
        let bytes = (MAX_SEQUENCE_ITEMS as i64 + 1).to_le_bytes();
        let mut r = Reader::new(&bytes);
        let err = r.count_i64("components").unwrap_err();
        assert!(err.starts_with("CIRCUIT_BAD_COUNT|components="), "got: {err}");
    }

    #[test]
    fn finish_rejects_trailing_bytes() {
        let bytes = [1u8, 2, 3];
        let mut r = Reader::new(&bytes);
        r.u8().unwrap();
        let err = r.finish().unwrap_err();
        assert!(err.starts_with("CIRCUIT_TRAILING|"), "got: {err}");
    }

    #[test]
    fn utf8_error_codes() {
        // length=2, payload=invalid UTF-8 continuation without lead
        let bytes = [0x02, 0x00, 0x80, 0x81];
        let mut r = Reader::new(&bytes);
        let err = r.string().unwrap_err();
        assert!(err.starts_with("CIRCUIT_UTF8|"), "got: {err}");
    }
}