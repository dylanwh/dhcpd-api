#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::expect_used
)]

#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

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
    body::Body,
    extract::{ConnectInfo, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use db::{Database, DB};
use model::{Device, MacAddr};
use serde_json::{json, Value};
use tokio::{sync::Mutex, net::TcpListener};

use crate::model::{FindByIp, FindByMac};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IPv6 not supported")]
    Ipv6NotSupported,

    #[error("Invalid IP address: {0}")]
    InvalidIpAddr(#[from] std::net::AddrParseError),

    #[error("Invalid MAC address: {0}")]
    InvalidMacAddr(#[from] macaddr::InvalidMacAddr),

    #[error("Database error: {0}")]
    Database(#[from] db::Error),

    #[error("Listen error: {0}")]
    Listen(#[from] std::io::Error),
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Error> {
    let args = Args::new();

    let db = Arc::new(Mutex::new(Database::new().await?));
    let tracker = TaskTracker::new();
    let shutdown = CancellationToken::new();

    let files_db = db.clone();
    let files_shutdown = shutdown.clone();
    tracker.spawn(async move {
        db::watch_files(
            files_db,
            files_shutdown,
            &args.dhcpd_config,
            &args.dhcpd_leases,
        )
        .await
        .unwrap_or_else(|e| {
            eprintln!("Error watching files: {e}");
        });
    });

    let router = Router::new()
        .route("/", get(index))
        .route("/whoami", get(whoami))
        .route("/ip/:ip", get(lookup_ip))
        .route("/mac/:mac", get(lookup_mac))
        .route("/vendors", get(vendors))
        .with_state(db);

    let listener = TcpListener::bind(args.listen).await.map_err(Error::Listen)?;
    let axum_shutdown = shutdown.clone();
    tracker.spawn(async move {
        let serve = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                axum_shutdown.cancelled().await;
            })
            .await;
        if let Err(e) = serve {
            tracing::error!("server error: {}", e);
        }
    });

    tracker.close();

    tokio::select! {
        () = shutdown_signal() => {
            tracing::info!("Shutting down...");
            shutdown.cancel();
        },
        () = shutdown.cancelled() => {
            tracing::info!("Shutting down...");
        },
    }

    tracker.wait().await;

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
) -> Result<Json<Value>, Error> {
    let client_ip = match addr.ip() {
        IpAddr::V4(client_ip) => Ok(client_ip),
        IpAddr::V6(_) => Err(Error::Ipv6NotSupported),
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
) -> Result<Json<Value>, Error> {
    let ip = Ipv4Addr::from_str(&ip)?;
    let db = db.lock().await;

    let leases = db.leases.find_by_ip(ip);
    let hosts = db.hosts.find_by_ip(ip);

    Ok(Json(json!({
        "devices": Device::from_leases_and_hosts(&leases, &hosts, &db.vendor_mapping),
    })))
}

async fn lookup_mac(State(db): State<DB>, Path(mac): Path<String>) -> Result<Json<Value>, Error> {
    let db = db.lock().await;
    let mac = mac.parse::<MacAddr>()?;

    let leases = db.leases.find_by_mac(&mac);
    let hosts = db.hosts.find_by_mac(&mac);

    Ok(Json(json!({
        "devices": Device::from_leases_and_hosts(&leases, &hosts, &db.vendor_mapping),
    })))
}

async fn vendors(State(db): State<DB>) -> Result<Json<Value>, Error> {
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

impl IntoResponse for Error {
    fn into_response(self) -> Response<Body> {
        let resp = match self {
            Error::Ipv6NotSupported => (StatusCode::BAD_REQUEST, "IPv6 not supported".to_string()),
            Error::InvalidIpAddr(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            Error::InvalidMacAddr(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
        };

        resp.into_response()
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        #[allow(clippy::expect_used)]
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        #[allow(clippy::expect_used)]
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
