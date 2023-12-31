use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use chrono::{DateTime, Utc};
use eyre::{Context, Result};
use tokio::sync::RwLock;

use crate::{
    dhcp_parsers::{hosts, leases},
    model::{Host, Lease},
    vendor_macs::VendorMapping,
};

#[derive(Clone)]
pub struct App {
    pub leases: Arc<RwLock<Vec<Lease>>>,
    pub hosts: Arc<RwLock<Vec<Host>>>,

    pub vendor_mapping: Arc<VendorMapping>,

    pub last_update_leases: Arc<RwLock<Option<DateTime<Utc>>>>,
    pub last_update_hosts: Arc<RwLock<Option<DateTime<Utc>>>>,
    pub last_update_check: Arc<RwLock<Option<DateTime<Utc>>>>,
}

pub async fn new() -> Result<App> {
    let leases = Arc::new(RwLock::new(Vec::new()));
    let hosts = Arc::new(RwLock::new(Vec::new()));
    let vendor_mapping = Arc::new(VendorMapping::fetch(true).await?);
    let last_update_leases = Arc::new(RwLock::new(None));
    let last_update_hosts = Arc::new(RwLock::new(None));
    let last_update_check = Arc::new(RwLock::new(None));
    let app = App {
        leases,
        hosts,
        vendor_mapping,
        last_update_leases,
        last_update_hosts,
        last_update_check,
    };

    Ok(app)
}

pub async fn watch_files(app: App, dhcpd_config: &PathBuf, dhcpd_leases: &PathBuf) -> Result<()> {
    let dhcpd_config = std::fs::canonicalize(dhcpd_config)?;
    let dhcpd_leases = std::fs::canonicalize(dhcpd_leases)?;

    update_leases(&app, &dhcpd_leases).await?;
    update_hosts(&app, &dhcpd_config).await?;

    tokio::spawn(async move {
        // check for changes every 60 seconds, using just mtime stat calls.
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            if file_changed(app.last_update_leases.clone(), &dhcpd_leases).await {
                if let Err(e) = update_leases(&app, &dhcpd_leases).await {
                    tracing::error!("Failed to update leases: {}", e);
                }
            }
            if file_changed(app.last_update_hosts.clone(), &dhcpd_config).await {
                if let Err(e) = update_hosts(&app, &dhcpd_config).await {
                    tracing::error!("Failed to update hosts: {}", e);
                }
            }
            app.last_update_check.write().await.replace(Utc::now());
            interval.tick().await;
        }
    });

    Ok(())
}

async fn file_changed(last_update: Arc<RwLock<Option<DateTime<Utc>>>>, file: &PathBuf) -> bool {
    let last_update = last_update.read().await;
    let Some(last_update) = *last_update else {
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

pub async fn update_leases<P>(app: &App, dhcpd_leases: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let dhcpd_leases = dhcpd_leases.as_ref();
    let buf = tokio::fs::read_to_string(dhcpd_leases)
        .await
        .wrap_err(format!(
            "Failed to read dhcpd leases: {}",
            dhcpd_leases.to_string_lossy()
        ))?;
    let new_leases = leases::parse(&buf)?;
    {
        let mut leases = app.leases.write().await;
        *leases = new_leases;
    }
    app.last_update_leases.write().await.replace(Utc::now());
    Ok(())
}

pub async fn update_hosts<P>(app: &App, dhcpd_config: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let dhcpd_config = dhcpd_config.as_ref();
    let buf = tokio::fs::read_to_string(dhcpd_config)
        .await
        .wrap_err(format!(
            "Failed to read dhcpd config: {}",
            dhcpd_config.to_string_lossy()
        ))?;
    let new_hosts = hosts::parse(&buf)?;
    let mut hosts = app.hosts.write().await;
    *hosts = new_hosts;
    app.last_update_hosts.write().await.replace(Utc::now());
    Ok(())
}
