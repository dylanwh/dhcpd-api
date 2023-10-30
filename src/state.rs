use std::{path::PathBuf, sync::Arc, time::Duration};

use async_watcher::{
    notify::{RecommendedWatcher, RecursiveMode},
    AsyncDebouncer,
};
use eyre::{Context, Result};
use tokio::sync::{Mutex, RwLock};

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

    debouncer: Arc<Mutex<Option<AsyncDebouncer<RecommendedWatcher>>>>,
}

pub async fn new() -> Result<App> {
    let leases = Arc::new(RwLock::new(Vec::new()));
    let hosts = Arc::new(RwLock::new(Vec::new()));
    let debouncer = Arc::new(Mutex::new(None));
    let vendor_mapping = Arc::new(VendorMapping::fetch(true).await?);
    let state = App {
        leases,
        hosts,
        vendor_mapping,
        debouncer,
    };

    Ok(state)
}

const WATCH_TIMEOUT: u64 = 2;
const WATCH_TICK_RATE: u64 = 2;

pub async fn watch_files(
    state: App,
    dhcpd_config: &PathBuf,
    dhcpd_leases: &PathBuf,
) -> Result<()> {
    let dhcpd_config = std::fs::canonicalize(dhcpd_config)?;
    let dhcpd_leases = std::fs::canonicalize(dhcpd_leases)?;

    update_leases(&state, &dhcpd_leases).await?;
    update_hosts(&state, &dhcpd_config).await?;

    let (mut debouncer, mut file_events) = AsyncDebouncer::new_with_channel(
        Duration::from_secs(WATCH_TIMEOUT),
        Some(Duration::from_secs(WATCH_TICK_RATE)),
    )
    .await?;
    debouncer
        .watcher()
        .watch(dhcpd_leases.as_ref(), RecursiveMode::NonRecursive)?;

    debouncer
        .watcher()
        .watch(dhcpd_config.as_ref(), RecursiveMode::NonRecursive)?;

    state.debouncer.lock().await.replace(debouncer);

    tokio::spawn(async move {
        while let Some(event) = file_events.recv().await {
            if let Ok(event) = event {
                for event in event {
                    if event.path == dhcpd_leases {
                        eprintln!("Updating leases");
                        if let Err(e) = update_leases(&state, &dhcpd_leases).await {
                            tracing::error!("Failed to update leases: {}", e);
                        }
                    } else if event.path == dhcpd_config {
                        eprintln!("Updating hosts");
                        if let Err(e) = update_hosts(&state, &dhcpd_config).await {
                            tracing::error!("Failed to update hosts: {}", e);
                        }
                    }
                }
            }
        }
    });

    Ok(())
}

pub async fn update_leases(state: &App, dhcpd_leases: &PathBuf) -> Result<()> {
    let buf = tokio::fs::read_to_string(dhcpd_leases)
        .await
        .wrap_err(format!(
            "Failed to read dhcpd leases: {}",
            dhcpd_leases.to_string_lossy()
        ))?;
    let new_leases = leases::parse(&buf)?;
    {
        let mut leases = state.leases.write().await;
        *leases = new_leases;
    }
    Ok(())
}

pub async fn update_hosts(state: &App, dhcpd_config: &PathBuf) -> Result<()> {
    let buf = tokio::fs::read_to_string(dhcpd_config)
        .await
        .wrap_err(format!(
            "Failed to read dhcpd config: {}",
            dhcpd_config.to_string_lossy()
        ))?;
    let new_hosts = hosts::parse(&buf)?;
    let mut hosts = state.hosts.write().await;
    *hosts = new_hosts;
    Ok(())
}
