#!/bin/sh

# PROVIDE: dhcpd_api
# REQUIRE: NETWORKING
# BEFORE: LOGIN
# KEYWORD: nojail

. /etc/rc.subr

name="dhcpd_api"
rcvar=dhcpd_api_enable

load_rc_config $name

pidfile="/var/run/${name}.pid"

: ${dhcpd_api_enable:=NO}
: ${dhcpd_api_flags:=""}

rtart_cmd="${name}_start"
status_cmd="${name}_status"
stop_cmd="${name}_stop"

extra_commands="status"

dhcpd_api_start()
{
    info "Starting ${name}."
    /usr/local/bin/dhcpd_api --write-pid=${pidfile} ${dhcpd_api_flags} | logger -t dhcpd_api &
}


dhcpd_api_status()
{
    if [ -f ${pidfile} ]; then
        pid=`cat ${pidfile}`
        if ps -p ${pid} | grep -q ${pid}; then
            echo "${name} is running as pid ${pid}."
        else
            echo "${name} is not running (pidfile exists)."
        fi
    else
        echo "${name} is not running."
    fi
}

dhcpd_api_stop()
{
    info "Stopping ${name}."
    if [ -f ${pidfile} ]; then
        pid=`cat ${pidfile}`
        if ps -p ${pid} | grep -q ${pid}; then
            kill ${pid}
        else
            echo "${name} is not running (pidfile exists)."
        fi
    else
        echo "${name} is not running."
    fi
}

run_rc_command "$1"
