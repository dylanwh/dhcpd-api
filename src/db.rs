use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use chrono::{DateTime, Utc};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::{
    dhcp_parsers::{self, hosts, leases},
    model::{Host, Lease},
    vendor_macs::VendorMapping,
};

pub type DB = Arc<Mutex<Database>>;

#[derive(Clone)]
pub struct Database {
    pub leases: Vec<Lease>,
    pub hosts: Vec<Host>,

    pub vendor_mapping: VendorMapping,

    pub last_update_leases: Option<DateTime<Utc>>,
    pub last_update_hosts: Option<DateTime<Utc>>,
    pub last_update_check: Option<DateTime<Utc>>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    VendorMacs(#[from] crate::vendor_macs::Error),

    #[error(transparent)]
    HostsParser(#[from] dhcp_parsers::HostsParseError),

    #[error(transparent)]
    LeasesParser(#[from] dhcp_parsers::LeasesParseError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl Database {
    pub async fn new() -> Result<Self, Error> {
        let leases = Vec::new();
        let hosts = Vec::new();
        let vendor_mapping = VendorMapping::fetch(true).await?;
        let last_update_leases = None;
        let last_update_hosts = None;
        let last_update_check = None;
        let db = Database {
            leases,
            hosts,
            vendor_mapping,
            last_update_leases,
            last_update_hosts,
            last_update_check,
        };

        Ok(db)
    }
}

pub async fn watch_files(
    db: DB,
    shutdown: CancellationToken,
    dhcpd_config: &PathBuf,
    dhcpd_leases: &PathBuf,
) -> Result<(), Error> {
    let dhcpd_config = std::fs::canonicalize(dhcpd_config)?;
    let dhcpd_leases = std::fs::canonicalize(dhcpd_leases)?;

    update_leases(db.clone(), &dhcpd_leases).await?;
    update_hosts(db.clone(), &dhcpd_config).await?;

    // check for changes every 60 seconds, using just mtime stat calls.
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    loop {
        tokio::select! {
            _ = interval.tick() => check_files(db.clone(), &dhcpd_config, &dhcpd_leases).await,
            () = shutdown.cancelled() => break,
        }
    }

    Ok(())
}

async fn check_files(db: DB, dhcpd_config: &PathBuf, dhcpd_leases: &PathBuf) {
    let (last_update_leases, last_update_hosts) = {
        let mut db = db.lock().await;
        db.last_update_check.replace(Utc::now());
        (db.last_update_leases, db.last_update_hosts)
    };

    if file_changed(last_update_leases, dhcpd_leases).await {
        update_leases(db.clone(), dhcpd_leases)
            .await
            .unwrap_or_else(|e| tracing::error!("Failed to update leases: {}", e));
    }
    if file_changed(last_update_hosts, dhcpd_config).await {
        update_hosts(db.clone(), dhcpd_config)
            .await
            .unwrap_or_else(|e| tracing::error!("Failed to update hosts: {}", e));
    }
}

async fn file_changed(last_update: Option<DateTime<Utc>>, file: &PathBuf) -> bool {
    let Some(last_update) = last_update else {
        return true;
    };

    let metadata = match tokio::fs::metadata(file).await {
        Ok(metadata) => metadata,
        Err(e) => {
            tracing::error!("Failed to stat file: {}", e);
            return false;
        }
    };

    let modified: DateTime<Utc> = match metadata.modified() {
        Ok(modified) => modified.into(),
        Err(e) => {
            tracing::error!("Failed to get modified time: {}", e);
            return false;
        }
    };

    modified > last_update
}

pub async fn update_leases<P>(db: DB, dhcpd_leases: P) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let dhcpd_leases = dhcpd_leases.as_ref();
    let buf = tokio::fs::read_to_string(dhcpd_leases).await?;
    let new_leases = leases::parse(&buf)?;
    {
        let mut db = db.lock().await;
        db.leases = new_leases;
        db.last_update_leases.replace(Utc::now());
    }
    Ok(())
}

pub async fn update_hosts<P>(db: DB, dhcpd_config: P) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let dhcpd_config = dhcpd_config.as_ref();
    let buf = tokio::fs::read_to_string(dhcpd_config).await?;
    let new_hosts = hosts::parse(&buf)?;
    {
        let mut db = db.lock().await;
        db.hosts = new_hosts;
        db.last_update_hosts.replace(Utc::now());
    }
    Ok(())
}
