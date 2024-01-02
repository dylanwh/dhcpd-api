# dhcpd-api

This is a simple API for extracting client information from the ISC DHCP server. It is designed to be used on OPNSense, but should work any environment that uses the ISC DHCP server.

This tool is designed to be used with [hostapd-api](https://github.com/dylanwh/hostapd-api),
and together they can be used to understand certain aspects of a network.

## Features

* Query DHCP static mappings
* Query DHCP leases
* Query mac address vendor name

## Getting Started

### Prerequisites

* Rust 1.75 or higher
* isc-dhcpd

### Installation

1. Clone the repository

```bash
git clone https://github.com/dylanwh/dhcpd-api.git
```

2. Build the project

```bash
cargo build --release
```

### Setting up as a Service on OPNSense / FreeBSD

1. Copy the service file from the `contrib/` directory to the `/usr/local/etc/rc.d/` directory. You can do this with the following command:

```bash
sudo cp contrib/freebsd/dhcpd_api.sh /usr/local/etc/rc.d/dhcpd_api
sudo cp target/release/dhcpd_api /usr/local/bin/dhcpd_api
sudo service dhcpd_api start
# make sure it starts on boot
sudo sysrc dhcpd_api_enable=YES
```


