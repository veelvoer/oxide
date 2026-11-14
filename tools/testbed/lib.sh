export PATH=$HOME/oxide-target/release:$HOME/.cargo/bin:$PATH CARGO_TARGET_DIR=$HOME/oxide-target
export T=$HOME/oxide-target/${BUILD:-release}
export HOST=oxhost
export OXIDE_SOCKET=/tmp/ox/oxide.sock OXIDE_TEST=1
export XDG_DATA_HOME=/tmp/ox/data XDG_CACHE_HOME=/tmp/ox/cache
host_up() {
  [ -f /tmp/ox/host.pid ] && kill -0 $(cat /tmp/ox/host.pid) 2>/dev/null && return
  kwin_wayland --virtual --no-lockscreen --width ${HW:-3840} --height ${HH:-1080} --socket $HOST >/tmp/ox/kwin.log 2>&1 &
  echo $! > /tmp/ox/host.pid
  sleep 3
}
bus_up() {
  if [ -f /tmp/ox/bus ] && [ -S "$(sed 's/unix:path=//; s/,.*//' /tmp/ox/bus)" ]; then export TESTBUS=$(cat /tmp/ox/bus); else
    TESTBUS=$(dbus-daemon --session --fork --print-address); echo $TESTBUS > /tmp/ox/bus; export TESTBUS; fi
  export DBUS_SESSION_BUS_ADDRESS=$TESTBUS
}
attach() {
  export XDG_CONFIG_HOME=${XDG_CONFIG_HOME:-/tmp/ox/cfg}
  [ -f /tmp/ox/bus ] && export TESTBUS=$(cat /tmp/ox/bus) && export DBUS_SESSION_BUS_ADDRESS=$TESTBUS
  export OX=$(sed 's/\x1b\[[0-9;]*m//g' /tmp/ox/ox.log 2>/dev/null | grep -ao 'name="wayland-[0-9]*"' | head -1 | cut -d'"' -f2)
}
Q() { WAYLAND_DISPLAY=$OX quickshell -p $XDG_CONFIG_HOME/oxide/active-shell ipc call "$@"; }
start_ox() {
  host_up; bus_up
  export XDG_CONFIG_HOME=${1:-/tmp/ox/cfg}
  rm -f $OXIDE_SOCKET
  WAYLAND_DISPLAY=$HOST OXIDE_NORMAL=1 OXIDE_SIZE=${2:-1920x1080} OXIDE_OUTPUTS=${3:-1} RUST_LOG=${OXRUST:-info} timeout ${LIFE:-3000} $T/oxide --winit >/tmp/ox/ox.log 2>&1 &
  echo $! > /tmp/ox/ox.pid
  for i in 1 2 3 4 5 6 7 8 9 10; do sleep 1; [ -S $OXIDE_SOCKET ] && break; done
  export OX=$(sed 's/\x1b\[[0-9;]*m//g' /tmp/ox/ox.log | grep -ao 'name="wayland-[0-9]*"' | head -1 | cut -d'"' -f2)
  sleep ${SETTLE:-4}
}
stop_ox() {
  $T/oxide-ctl quit >/dev/null 2>&1; sleep 1
  [ -f /tmp/ox/ox.pid ] && kill $(cat /tmp/ox/ox.pid) 2>/dev/null
  for p in $(pgrep -f "^quickshell -p /tmp/ox"); do kill $p 2>/dev/null; done
}
attach
