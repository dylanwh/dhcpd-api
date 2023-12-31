use std::net::Ipv4Addr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use crate::macaddr::MacAddr;
use crate::vendor_macs::VendorMapping;

pub type LeaseTime = Option<DateTime<Utc>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    pub address: Ipv4Addr,
    pub starts: LeaseTime,
    pub ends: LeaseTime,
    pub tstp: LeaseTime,
    pub cltt: LeaseTime,
    pub hardware_ethernet: MacAddr,
    pub client_hostname: Option<String>,
}

impl Lease {
    pub fn is_expired(&self) -> bool {
        if let Some(ends) = self.ends {
            ends < Utc::now()
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Host {
    pub fixed_address: Ipv4Addr,
    pub hardware_ethernet: MacAddr,
    pub hostname: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LeaseType {
    #[serde(rename = "active")]
    Active { since: LeaseTime, until: LeaseTime },
    #[serde(rename = "expired")]
    Expired { since: LeaseTime },
    #[serde(rename = "static")]
    Static,
}

#[derive(Debug, Serialize)]
pub struct Device<'a> {
    address: &'a Ipv4Addr,

    hardware_ethernet: &'a MacAddr,

    #[serde(skip_serializing_if = "Option::is_none")]
    hostname: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    vendor: Option<&'a str>,

    lease: LeaseType,

    #[serde(skip_serializing_if = "Option::is_none")]
    last_seen: LeaseTime,
}

impl<'a> Device<'a> {
    pub fn from_lease(lease: &'a Lease, vendor_mapping: &'a VendorMapping) -> Self {
        let lease_type = if lease.is_expired() {
            LeaseType::Expired { since: lease.ends }
        } else {
            LeaseType::Active {
                since: lease.starts,
                until: lease.ends,
            }
        };

        let vendor = vendor_mapping.get_vendor_name(&lease.hardware_ethernet);

        Self {
            address: &lease.address,
            hardware_ethernet: &lease.hardware_ethernet,
            hostname: lease.client_hostname.as_deref(),
            vendor,
            lease: lease_type,
            last_seen: lease.cltt,
        }
    }

    pub fn from_host(host: &'a Host, vendor_mapping: &'a VendorMapping) -> Self {
        let vendor = vendor_mapping.get_vendor_name(&host.hardware_ethernet);

        Self {
            address: &host.fixed_address,
            hardware_ethernet: &host.hardware_ethernet,
            hostname: host.hostname.as_deref(),
            vendor,
            lease: LeaseType::Static,
            last_seen: None,
        }
    }

    pub fn from_leases_and_hosts(
        leases: &'a [&Lease],
        hosts: &'a [&Host],
        vendor_mapping: &'a VendorMapping,
    ) -> Vec<Self> {
        let mut devices = Vec::with_capacity(leases.len() + hosts.len());

        for lease in leases {
            devices.push(Self::from_lease(lease, vendor_mapping));
        }

        for host in hosts {
            devices.push(Self::from_host(host, vendor_mapping));
        }

        devices
    }
}

pub trait FindByIp {
    type Item;

    fn find_by_ip(&self, ip: Ipv4Addr) -> Vec<&Self::Item>;
}

pub trait FindByMac {
    type Item;

    fn find_by_mac(&self, mac: &MacAddr) -> Vec<&Self::Item>;
}

impl FindByIp for Vec<Lease> {
    type Item = Lease;

    fn find_by_ip(&self, ip: Ipv4Addr) -> Vec<&Self::Item> {
        self.iter().filter(|lease| lease.address == ip).collect()
    }
}

impl FindByMac for Vec<Lease> {
    type Item = Lease;

    fn find_by_mac(&self, mac: &MacAddr) -> Vec<&Self::Item> {
        self.iter()
            .filter(|lease| lease.hardware_ethernet == *mac)
            .collect()
    }
}

impl FindByIp for Vec<Host> {
    type Item = Host;

    fn find_by_ip(&self, ip: Ipv4Addr) -> Vec<&Self::Item> {
        self.iter()
            .filter(|host| host.fixed_address == ip)
            .collect()
    }
}

impl FindByMac for Vec<Host> {
    type Item = Host;

    fn find_by_mac(&self, mac: &MacAddr) -> Vec<&Self::Item> {
        self.iter()
            .filter(|host| host.hardware_ethernet == *mac)
            .collect()
    }
}
