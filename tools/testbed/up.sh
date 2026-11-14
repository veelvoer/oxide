source /tmp/ox/lib.sh
stop_ox 2>/dev/null
SZ=${1:-1920x1080}; N=${2:-1}; CFG=${3:-/tmp/ox/cfg}
W=${SZ%x*}
if [ -f /tmp/ox/host.pid ]; then kill $(cat /tmp/ox/host.pid) 2>/dev/null; sleep 1; rm -f /tmp/ox/host.pid; fi
HW=$W HH=1080 start_ox $CFG $SZ $N
