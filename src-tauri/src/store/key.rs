//! SQLCipher 密钥管理：从 OS Keychain 取/生成 32B key + 16B salt。

use anyhow::Result;

const KEY_LEN: usize = 32;
const SALT_LEN: usize = 16;
const KEYCHAIN_SERVICE: &str = "weft";
const KEYCHAIN_ACCOUNT: &str = "db-key-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SqlCipherKey {
    key: [u8; KEY_LEN],
    salt: [u8; SALT_LEN],
}

impl SqlCipherKey {
    pub fn random() -> Self {
        use rand::RngCore;
        let mut buf = [0u8; KEY_LEN + SALT_LEN];
        rand::rngs::OsRng.fill_bytes(&mut buf);
        let mut key = [0u8; KEY_LEN];
        let mut salt = [0u8; SALT_LEN];
        key.copy_from_slice(&buf[..KEY_LEN]);
        salt.copy_from_slice(&buf[KEY_LEN..]);
        Self { key, salt }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != KEY_LEN + SALT_LEN {
            return Err(anyhow::anyhow!(
                "sqlcipher key must be {} bytes, got {}",
                KEY_LEN + SALT_LEN,
                bytes.len()
            ));
        }
        let mut key = [0u8; KEY_LEN];
        let mut salt = [0u8; SALT_LEN];
        key.copy_from_slice(&bytes[..KEY_LEN]);
        salt.copy_from_slice(&bytes[KEY_LEN..]);
        Ok(Self { key, salt })
    }

    pub fn to_bytes(&self) -> [u8; KEY_LEN + SALT_LEN] {
        let mut out = [0u8; KEY_LEN + SALT_LEN];
        out[..KEY_LEN].copy_from_slice(&self.key);
        out[KEY_LEN..].copy_from_slice(&self.salt);
        out
    }
}

/// Format key + salt as the SQLCipher `x'<64hex><32hex>'` literal.
/// See https://www.zetetic.net/sqlcipher/sqlcipher-api/#example-3-raw-key-data-with-explicit-salt-without-key-derivation
pub fn format_for_pragma(k: &SqlCipherKey) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(2 + 64 + 32 + 1);
    s.push_str("x'");
    for b in k.key.iter().chain(k.salt.iter()) {
        let _ = write!(s, "{:02x}", b);
    }
    s.push('\'');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_produces_48_bytes() {
        let k = SqlCipherKey::random();
        assert_eq!(k.to_bytes().len(), 48);
    }

    #[test]
    fn from_bytes_roundtrip() {
        let raw = [0xABu8; 48];
        let k = SqlCipherKey::from_bytes(&raw).unwrap();
        assert_eq!(k.to_bytes(), raw);
    }

    #[test]
    fn from_bytes_rejects_wrong_length() {
        assert!(SqlCipherKey::from_bytes(&[0u8; 47]).is_err());
        assert!(SqlCipherKey::from_bytes(&[0u8; 49]).is_err());
    }

    #[test]
    fn format_for_pragma_is_x_hex_literal() {
        let raw = {
            let mut v = [0u8; 48];
            for (i, b) in v.iter_mut().enumerate() {
                *b = i as u8;
            }
            v
        };
        let k = SqlCipherKey::from_bytes(&raw).unwrap();
        let s = format_for_pragma(&k);
        assert!(s.starts_with("x'"));
        assert!(s.ends_with('\''));
        // 64 hex chars (key) + 32 hex chars (salt) + x'' = 99
        assert_eq!(s.len(), 99);
        // first key byte 00 → "00"; second 01 → "01"
        assert_eq!(&s[2..6], "0001");
    }
}
