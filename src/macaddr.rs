use std::{fmt, str::FromStr, num::ParseIntError};

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

#[derive(Debug, thiserror::Error)]
pub enum InvalidMacAddr {
    #[error("mac address too short")]
    Short,

    #[error("mac address too long")]
    Long,

    #[error("mac address segment not two hex digits")]
    BadSegment,

    #[error("mac address parse error: {0}")]
    Parse(#[from] ParseIntError),
}

#[derive(Debug, thiserror::Error)]
pub enum InvalidMacPrefix {
    #[error("mac prefix segment contains non-hex character: {0}")]
    BadChar(char),

    #[error("mac prefix too long")]
    Long,

    #[error("mac prefix segment too long")]
    LongSegment,
}

impl FromStr for MacAddr {
    type Err = InvalidMacAddr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 6];
        let mut len = 0usize;
        for (i, byte) in s.split(':').enumerate() {
            if i >= bytes.len()  {
                return Err(InvalidMacAddr::Long);
            } else if byte.len() != 2 {
                return Err(InvalidMacAddr::BadSegment);
            }
            bytes[i] = u8::from_str_radix(byte, 16)?;
            len += 1;
        }
        if len != bytes.len() {
            return Err(InvalidMacAddr::Short);
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
    type Err = InvalidMacPrefix;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut nibs = Nibblet::new();
        for byte in s.split(':') {
            if byte.len() > 2 {
                return Err(InvalidMacPrefix::LongSegment);
            }
            for nib in byte.as_bytes() {
                let nib = match nib {
                    b'0'..=b'9' => *nib - b'0',
                    b'a'..=b'f' => *nib - b'a' + 10,
                    b'A'..=b'F' => *nib - b'A' + 10,
                    _ => return Err(InvalidMacPrefix::BadChar(*nib as char)),
                };
                nibs.push(nib);
            }
        }
        if nibs.len() > 12 {
            return Err(InvalidMacPrefix::Long);
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

    #[test]
    fn test_invalid_prefix() {
        let mac = "1g:20";
        let e = MacPrefix::from_str(mac).expect_err("Invalid MAC prefix");
        match e {
            InvalidMacPrefix::BadChar(c) => assert_eq!(c, 'g'),
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_long_prefix() {
        let mac = "10:20:30:40:50:60:70:80:90:10:20:30:40:50:60:70:80:90:10:20:30:40:50:60:70:80:90:10:20:30:40:50:60:70:80:90:10:20:30:40:50:60:70:80:90:10:20:30:40:50:60:70:80:90";
        let e = MacPrefix::from_str(mac).expect_err("Invalid MAC prefix");
        match e {
            InvalidMacPrefix::Long => (),
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_long_segment_prefix() {
        let mac = "10:20:30:40:50:600";
        let e = MacPrefix::from_str(mac).expect_err("Invalid MAC prefix");
        match e {
            InvalidMacPrefix::LongSegment => (),
            _ => unreachable!(),
        }
    }
}
