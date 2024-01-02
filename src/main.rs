#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::expect_used
)]

#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

mod args;
mod db;
mod dhcp_parsers;
mod macaddr;
mod model;
mod vendor_macs;

use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
    sync::Arc,
};

use args::Args;

use axum::{
    extract::{ConnectInfo, Path, State},
    routing::get,
    Json, Router,
};
use db::{Database, DB};
use eyre::{eyre, Result};
use model::{Device, MacAddr};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::model::{FindByIp, FindByMac};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = Args::new();

    let db = Arc::new(Mutex::new(Database::new().await?));
    db::watch_files(db.clone(), &args.dhcpd_config, &args.dhcpd_leases).await?;

    let router = Router::new()
        .route("/", get(index))
        .route("/whoami", get(whoami))
        .route("/ip/:ip", get(lookup_ip))
        .route("/mac/:mac", get(lookup_mac))
        .route("/vendors", get(vendors))
        .with_state(db);

    axum::Server::bind(&args.listen)
        .serve(router.into_make_service_with_connect_info::<SocketAddr>())
        .await?;

    Ok(())
}

async fn index(State(db): State<DB>) -> Json<Value> {
    let db = db.lock().await;
    let mut devices = Vec::with_capacity(db.leases.len() + db.hosts.len());

    for lease in &db.leases {
        devices.push(Device::from_lease(lease, &db.vendor_mapping));
    }

    for host in &db.hosts {
        devices.push(Device::from_host(host, &db.vendor_mapping));
    }

    Json(json!({
        "devices": devices,
        "last_update": {
            "leases": db.last_update_leases,
            "hosts": db.last_update_hosts,
            "check": db.last_update_check,
        }
    }))
}

async fn whoami(
    State(db): State<DB>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> axum_error::Result<Json<Value>> {
    let client_ip = match addr.ip() {
        IpAddr::V4(client_ip) => Ok(client_ip),
        IpAddr::V6(_) => Err(eyre!("IPv6 not supported")),
    }?;
    let db = db.lock().await;

    let leases = db.leases.find_by_ip(client_ip);
    let hosts = db.hosts.find_by_ip(client_ip);

    Ok(Json(json!({
        "devices": Device::from_leases_and_hosts(&leases, &hosts, &db.vendor_mapping),
    })))
}

async fn lookup_ip(
    State(db): State<DB>,
    Path(ip): Path<String>,
) -> axum_error::Result<Json<Value>> {
    let ip = Ipv4Addr::from_str(&ip).map_err(|_| eyre!("Invalid IP address: {ip}"))?;
    let db = db.lock().await;

    let leases = db.leases.find_by_ip(ip);
    let hosts = db.hosts.find_by_ip(ip);

    Ok(Json(json!({
        "devices": Device::from_leases_and_hosts(&leases, &hosts, &db.vendor_mapping),
    })))
}

async fn lookup_mac(
    State(db): State<DB>,
    Path(mac): Path<String>,
) -> axum_error::Result<Json<Value>> {
    let db = db.lock().await;
    let mac = mac
        .parse::<MacAddr>()
        .map_err(|_| eyre!("Invalid MAC address"))?;

    let leases = db.leases.find_by_mac(&mac);
    let hosts = db.hosts.find_by_mac(&mac);

    Ok(Json(json!({
        "devices": Device::from_leases_and_hosts(&leases, &hosts, &db.vendor_mapping),
    })))
}

async fn vendors(State(db): State<DB>) -> axum_error::Result<Json<Value>> {
    let db = db.lock().await;
    let mut vendors = BTreeSet::new();

    for lease in &db.leases {
        if let Some(vendor) = db.vendor_mapping.get_vendor_name(&lease.hardware_ethernet) {
            vendors.insert(vendor);
        }
    }
    for host in &db.hosts {
        if let Some(vendor) = db.vendor_mapping.get_vendor_name(&host.hardware_ethernet) {
            vendors.insert(vendor);
        }
    }

    Ok(Json(json!({
        "vendors": vendors,
    })))
}
