#!/bin/sh

# PROVIDE: dhcpd_api
# REQUIRE: NETWORKING
# BEFORE: LOGIN
# KEYWORD: nojail

# shellcheck disable=SC1091
. /etc/rc.subr

name="dhcpd_api"
# shellcheck disable=SC2034
rcvar=dhcpd_api_enable

load_rc_config $name


: "${dhcpd_api_enable:=NO}"
: "${dhcpd_api_pidfile:="/var/run/dhcpd_api.pid"}"


pidfile="${dhcpd_api_pidfile}"

# shellcheck disable=SC2034
procname="daemon"

# shellcheck disable=SC2034
start_cmd="dhcpd_api_start"

dhcpd_api_start()
{
    daemon -S -r -f -u dhcpd_api -P "$pidfile" -R 10 /usr/local/bin/dhcpd-api &
}

run_rc_command "$1"
