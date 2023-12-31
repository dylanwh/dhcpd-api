use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Args {
    #[arg(long, default_value = "/var/dhcpd/var/db/dhcpd.leases")]
    pub dhcpd_leases: PathBuf,

    #[arg(long, default_value = "/var/dhcpd/etc/dhcpd.conf")]
    pub dhcpd_config: PathBuf,

    #[arg(short, long, default_value = "0.0.0.0:16768")]
    pub listen: SocketAddr,
}

impl Args {
    pub fn new() -> Self {
        Self::parse()
    }
}
