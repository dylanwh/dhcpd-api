mod args;
mod dhcp_parsers;
mod model;

use std::time::Duration;

use args::Args;

use async_watcher::{AsyncDebouncer, notify::RecursiveMode};
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::new();

    let (mut debouncer, mut file_events) =
        AsyncDebouncer::new_with_channel(Duration::from_secs(5), Some(Duration::from_secs(5)))
            .await?;

    debouncer
        .watcher()
        .watch(args.dhcpd_leases.as_ref(), RecursiveMode::NonRecursive)?;

    debouncer
        .watcher()
        .watch(args.dhcpd_config.as_ref(), RecursiveMode::NonRecursive)?;

    while let Some(event) = file_events.recv().await {
        if let Ok(event) = event {
            for event in event {
                println!("{:?}", event.path);
            }
        }
    }

    // let leases = std::fs::read_to_string("./dhcpd.leases").unwrap();
    // let leases = dhcp_parsers::leases::parse(&leases).unwrap();

    // let hosts = std::fs::read_to_string("./dhcpd.conf").unwrap();
    // let hosts = dhcp_parsers::hosts::parse(&hosts).unwrap();

    // let json = json!({
    //     "leases": leases,
    //     "hosts": hosts,
    // });
    // println!("{}", json);

    Ok(())
}
