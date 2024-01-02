#!/bin/sh
# this is a little thing I use to rebuild on my opnsense box
cargo build --release
sudo service dhcpd_api stop
sudo cp target/release/dhcpd-api /usr/local/bin/
sudo service dhcpd_api start
