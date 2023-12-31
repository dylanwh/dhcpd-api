#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::expect_used
)]

mod args;
mod dhcp_parsers;
mod macaddr;
mod model;
mod state;
mod vendor_macs;

use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
};

use args::Args;

use axum::{
    extract::{ConnectInfo, Path, State},
    routing::get,
    Json, Router,
};
use eyre::{eyre, Result};
use model::{Device, MacAddr};
use serde_json::json;
use state::App;

use crate::model::{FindByIp, FindByMac};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = Args::new();

    let state = state::new().await?;
    state::watch_files(state.clone(), &args.dhcpd_config, &args.dhcpd_leases).await?;

    let router = Router::new()
        .route("/", get(index))
        .route("/whoami", get(whoami))
        .route("/ip/:ip", get(lookup_ip))
        .route("/mac/:mac", get(lookup_mac))
        .route("/vendors", get(vendors))
        .with_state(state.clone());

    axum::Server::bind(&args.listen)
        .serve(router.into_make_service_with_connect_info::<SocketAddr>())
        .await?;

    Ok(())
}

async fn index(State(state): State<App>) -> Json<serde_json::Value> {
    let mut devices = Vec::new();

    {
        let leases = state.leases.read().await;
        for lease in leases.iter() {
            devices.push(Device::from_lease(lease, &state.vendor_mapping));
        }
    }

    {
        let hosts = state.hosts.read().await;
        for host in hosts.iter() {
            devices.push(Device::from_host(host, &state.vendor_mapping));
        }
    }

    Json(json!({
        "devices": devices,
        "last_update": {
            "leases": state.last_update_leases.read().await.map(|d| d.to_rfc3339()),
            "hosts": state.last_update_hosts.read().await.map(|d| d.to_rfc3339()),
            "check": state.last_update_check.read().await.map(|d| d.to_rfc3339()),
        }
    }))
}

async fn whoami(
    State(state): State<App>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> axum_error::Result<Json<serde_json::Value>> {
    let client_ip = match addr.ip() {
        IpAddr::V4(client_ip) => Ok(client_ip),
        IpAddr::V6(_) => Err(eyre!("IPv6 not supported")),
    }?;
    let leases = state.leases.read().await;
    let hosts = state.hosts.read().await;

    let leases = leases.find_by_ip(client_ip);
    let hosts = hosts.find_by_ip(client_ip);

    Ok(Json(json!({
        "devices": Device::from_leases_and_hosts(&leases, &hosts, &state.vendor_mapping),
    })))
}

async fn lookup_ip(
    State(state): State<App>,
    Path(ip): Path<String>,
) -> axum_error::Result<Json<serde_json::Value>> {
    let ip = Ipv4Addr::from_str(&ip).map_err(|_| eyre!("Invalid IP address: {ip}"))?;
    let leases = state.leases.read().await;
    let hosts = state.hosts.read().await;

    let leases = leases.find_by_ip(ip);
    let hosts = hosts.find_by_ip(ip);

    Ok(Json(json!({
        "devices": Device::from_leases_and_hosts(&leases, &hosts, &state.vendor_mapping),
    })))
}

async fn lookup_mac(
    State(state): State<App>,
    Path(mac): Path<String>,
) -> axum_error::Result<Json<serde_json::Value>> {
    let mac = mac
        .parse::<MacAddr>()
        .map_err(|_| eyre!("Invalid MAC address"))?;

    let leases = state.leases.read().await;
    let hosts = state.hosts.read().await;

    let leases = leases.find_by_mac(&mac);
    let hosts = hosts.find_by_mac(&mac);

    Ok(Json(json!({
        "devices": Device::from_leases_and_hosts(&leases, &hosts, &state.vendor_mapping),
    })))
}

async fn vendors(State(state): State<App>) -> axum_error::Result<Json<serde_json::Value>> {
    let mut vendors = BTreeSet::new();
    for lease in state.leases.read().await.iter() {
        if let Some(vendor) = state
            .vendor_mapping
            .get_vendor_name(&lease.hardware_ethernet)
        {
            vendors.insert(vendor.to_string());
        }
    }
    for host in state.hosts.read().await.iter() {
        if let Some(vendor) = state
            .vendor_mapping
            .get_vendor_name(&host.hardware_ethernet)
        {
            vendors.insert(vendor.to_string());
        }
    }

    Ok(Json(json!({
        "vendors": vendors,
    })))
}
