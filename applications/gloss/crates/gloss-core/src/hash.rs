use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// blake3 of the file's exact bytes. JSON form is a 64-char hex string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentHash(pub [u8; 32]);

impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        ContentHash::from_hex(&s).ok_or_else(|| serde::de::Error::custom("invalid content hash"))
    }
}

impl ContentHash {
    pub fn to_hex(self) -> String {
        hex_encode(&self.0)
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        let raw = hex_decode(s)?;
        let arr: [u8; 32] = raw.try_into().ok()?;
        Some(ContentHash(arr))
    }
}

pub fn hash_bytes(bytes: &[u8]) -> ContentHash {
    ContentHash(*blake3::hash(bytes).as_bytes())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = from_hex(bytes[i])?;
        let lo = from_hex(bytes[i + 1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let h = hash_bytes(b"hello");
        assert_eq!(ContentHash::from_hex(&h.to_hex()), Some(h));
    }

    #[test]
    fn json_is_hex_string() {
        let h = hash_bytes(b"hello");
        let raw = serde_json::to_string(&h).unwrap();
        assert_eq!(raw, format!("\"{}\"", h.to_hex()));
        assert_eq!(serde_json::from_str::<ContentHash>(&raw).unwrap(), h);
    }

    #[test]
    fn different_bytes_different_hash() {
        assert_ne!(hash_bytes(b"a"), hash_bytes(b"b"));
    }
}
