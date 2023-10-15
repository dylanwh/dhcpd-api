use serde_json::json;

mod dhcp_parsers;
mod model;

fn main() {
    let leases = std::fs::read_to_string("./dhcpd.leases").unwrap();
    let leases = dhcp_parsers::leases::parse(&leases).unwrap();

    let hosts = std::fs::read_to_string("./dhcpd.conf").unwrap();
    let hosts = dhcp_parsers::hosts::parse(&hosts).unwrap();

    let json = json!({
        "leases": leases,
        "hosts": hosts,
    });
    println!("{}", json);
}
