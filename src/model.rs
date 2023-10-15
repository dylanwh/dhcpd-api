use std::{net::Ipv4Addr, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/*
lease 10.0.1.199 {
  starts 0 2022/11/20 21:27:34;
  ends 0 2022/11/20 21:29:34;
  tstp 0 2022/11/20 21:29:34;
  cltt 0 2022/11/20 21:27:34;
  hardware ethernet 12:6d:88:95:58:89;
}
*/

pub type LeaseTime = Option<DateTime<Utc>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Lease {
    pub address: Ipv4Addr,
    pub starts: LeaseTime,
    pub ends: LeaseTime,
    pub tstp: LeaseTime,
    pub cltt: LeaseTime,
    pub hardware_ethernet: MacAddr,
    pub client_hostname: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Host {
    pub fixed_address: Ipv4Addr,
    pub hardware_ethernet: MacAddr,
    pub hostname: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MacAddr([u8; 6]);

impl From<[u8; 6]> for MacAddr {
    fn from(bytes: [u8; 6]) -> Self {
        MacAddr(bytes)
    }
}

impl FromStr for MacAddr {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 6];
        let mut i = 0;
        for byte in s.split(':') {
            if i >= 6 {
                return Err(format!("Invalid MAC address: {}", s));
            }
            bytes[i] =
                u8::from_str_radix(byte, 16).map_err(|e| format!("Invalid MAC address: {}", e))?;
            i += 1;
        }
        if i != 6 {
            return Err(format!("Invalid MAC address: {}", s));
        }
        Ok(MacAddr(bytes))
    }
}

impl Serialize for MacAddr {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let s = format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        );
        serializer.serialize_str(&s)
    }
}

// do Deserialize
impl<'de> Deserialize<'de> for MacAddr {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        MacAddr::from_str(&s).map_err(serde::de::Error::custom)
    }
}


