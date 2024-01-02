use std::{fmt, str::FromStr};

use eyre::{Context, Result};
use nibble_vec::Nibblet;
use radix_trie::TrieKey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct MacAddr([u8; 6]);
impl MacAddr {
    #[inline]
    pub fn bytes(&self) -> &[u8] {
        &self.0[..]
    }
}

impl From<[u8; 6]> for MacAddr {
    fn from(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }
}

impl FromStr for MacAddr {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 6];
        let mut len = 0usize;
        for (i, byte) in s.split(':').enumerate() {
            if i >= bytes.len() || byte.len() != 2 {
                return Err(eyre::eyre!(
                    "Invalid MAC address: {} (bytes: {:?})",
                    s,
                    bytes
                ));
            }
            bytes[i] =
                u8::from_str_radix(byte, 16).wrap_err("parse_mac_prefix: invalid hex byte")?;
            len += 1;
        }
        if len != bytes.len() {
            return Err(eyre::eyre!("Invalid MAC address: {} (wrong len)", s));
        }

        Ok(Self(bytes))
    }
}

impl<'de> Deserialize<'de> for MacAddr {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for MacAddr {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mac = self.0;
        let s = format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        );
        serializer.serialize_str(&s)
    }
}

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mac = self.0;
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MacPrefix(Nibblet);

impl From<&MacAddr> for MacPrefix {
    fn from(mac: &MacAddr) -> Self {
        Self(Nibblet::from_byte_vec(mac.bytes().to_vec()))
    }
}

impl FromStr for MacPrefix {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self> {
        let mut nibs = Nibblet::new();
        for byte in s.split(':') {
            for nib in byte.as_bytes() {
                let nib = match nib {
                    b'0'..=b'9' => *nib - b'0',
                    b'a'..=b'f' => *nib - b'a' + 10,
                    b'A'..=b'F' => *nib - b'A' + 10,
                    _ => return Err(eyre::eyre!("Invalid hex digit: {}", nib)),
                };
                nibs.push(nib);
            }
        }

        Ok(Self(nibs))
    }
}

impl TrieKey for MacPrefix {
    fn encode(&self) -> Nibblet {
        self.0.clone()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn test_short_macaddr() {
        let mac = "10:20:30";
        let _ = MacAddr::from_str(mac).expect_err("Invalid MAC address");
    }

    #[test]
    fn test_long_macaddr() {
        let mac = "10:20:30:40:50:60:70";
        let _ = MacAddr::from_str(mac).expect_err("Invalid MAC address");
    }

    #[test]
    fn test_short_segment_macaddr() {
        let mac = "10:20:30:40:5:60";
        let _ = MacAddr::from_str(mac).expect_err("Invalid MAC address");
    }

    #[test]
    fn test_long_segment_macaddr() {
        let mac = "10:20:30:40:50:600";
        let _ = MacAddr::from_str(mac).expect_err("Invalid MAC address");
    }

    #[test]
    fn test_invalid_hex() {
        let mac = "10:20:30:40:50:6g";
        let _ = MacAddr::from_str(mac).expect_err("Invalid MAC address");
    }
}
