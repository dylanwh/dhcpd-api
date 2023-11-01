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

# shellcheck disable=SC2034
start_precmd="dhcpd_api_pre_start"

dhcpd_api_pre_start()
{
    if ! id -u dhcpd_api >/dev/null 2>&1; then
        pw adduser dhcpd_api -g dhcpd -m -d /var/db/dhcpd-api -s /usr/sbin/nologin -c 'dhcpd api user'
    fi
}

dhcpd_api_start()
{
    if ! id -u dhcpd_api >/dev/null 2>&1; then
        err 1 "User dhcpd_api does not exist."
    fi
    if ! pw user show dhcpd_api 2>/dev/null |cut -d: -f9 |grep -q /var/db/dhcpd-api; then
        err 2 "User dhcpd_api has no home directory set."
    fi
    daemon -S -r -u dhcpd_api -P "$pidfile" -R 10 /usr/local/bin/dhcpd-api &
}

run_rc_command "$1"
