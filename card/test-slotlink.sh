#!/bin/sh
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
SCRIPT="$HERE/System/slotlink.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT HUP INT TERM
FAILS=0

fail() {
	echo "FAIL: $*"
	FAILS=$((FAILS + 1))
}

make_fake() {
	name="$1"
	out="$2"
	shift 2
	{
		echo '#!/bin/sh'
		echo "printf '%s\\n' \"\$*\" >> \"$out\""
		for line in "$@"; do echo "$line"; done
		echo 'exit 0'
	} > "$TMP/bin/$name"
	chmod 755 "$TMP/bin/$name"
}

reset() {
	rm -rf "$TMP/net" "$TMP/run" "$TMP/ctrl" "$TMP"/*.log "$TMP/bin"
	mkdir -p "$TMP/net/wlan0" "$TMP/net/wlan1" "$TMP/run" "$TMP/ctrl" "$TMP/modules" "$TMP/bin"
	: > "$TMP/modules/8821cs.ko"
	if [ "$1" = yes ]; then
		echo "8821cs 2863104 0 - Live 0x0" > "$TMP/proc-modules"
	else
		: > "$TMP/proc-modules"
	fi
	make_fake wpa_cli "$TMP/cli.log" 'echo "wpa_state=${FAKE_STATE:-COMPLETED}"'
	make_fake wpa_supplicant "$TMP/sup.log"
	make_fake ip "$TMP/ip.log"
	make_fake insmod "$TMP/mod.log"
	make_fake legacy "$TMP/legacy.log"
	make_fake udhcpc "$TMP/dhcp.log"
	printf 'network={\n\tssid="Home"\n\tpsk="hunter22"\n}\n' > "$TMP/wifi.conf"
}

run() {
	shift
	PATH="$TMP/bin:$PATH" \
		AGS_MODULES="$TMP/modules" AGS_NET_SYS="$TMP/net" AGS_RUN="$TMP/run" \
		AGS_CTRL_DIR="$TMP/ctrl" AGS_PROC_MODULES="$TMP/proc-modules" \
		AGS_WPA_SUPPLICANT="$TMP/bin/wpa_supplicant" AGS_WPA_CLI="$TMP/bin/wpa_cli" \
		AGS_IP="$TMP/bin/ip" AGS_INSMOD="$TMP/bin/insmod" AGS_UDHCPC="$TMP/bin/udhcpc" \
		AGS_UPDATE_WAIT_S="${WAIT_S:-1}" \
		AGS_LINK_WAIT_S="${WAIT_S:-1}" AGS_LINK_PIN_WAIT_S="${WAIT_S:-1}" \
		sh "$SCRIPT" "$@"
}

reset yes
run absent link host && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "host exited $rc"
grep -q 'ap_scan=2' "$TMP/run/slotlink-ap.conf" || fail "host AP scans before starting"
grep -q 'frequency=5745' "$TMP/run/slotlink-ap.conf" || fail "host AP not on 5745"
grep -q 'mode=2' "$TMP/run/slotlink-ap.conf" || fail "host config is not an AP"
grep -q 'addr add 10.42.0.1/24 dev wlan1' "$TMP/ip.log" || fail "host not addressed"
[ -f "$TMP/run/slotlink.session" ] || fail "host left no session mark"

reset yes
export FAKE_STATE=SCANNING
run absent link host && rc=0 || rc=$?
unset FAKE_STATE
[ "$rc" = 1 ] || fail "dead AP exited $rc, want 1"
grep -q 'addr add' "$TMP/ip.log" 2>/dev/null && fail "dead AP was addressed"

reset yes
run absent link join && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "join exited $rc"
grep -q 'scan_freq=5745' "$TMP/run/slotlink-sta.conf" || fail "join not pinned"
grep -q 'addr add 10.42.0.2/24 dev wlan0' "$TMP/ip.log" || fail "join not addressed"

reset yes
export FAKE_STATE=SCANNING
run absent link join && rc=0 || rc=$?
unset FAKE_STATE
[ "$rc" = 3 ] || fail "join with no host exited $rc, want 3"
[ "$(grep -c -- '-i wlan0' "$TMP/sup.log")" = 2 ] || fail "join did not make two attempts"

reset yes
run absent link join >/dev/null
run absent link down && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "down exited $rc"
for f in slotlink-ap.conf slotlink-sta.conf slotlink.session; do
	[ -e "$TMP/run/$f" ] && fail "down left $f"
done

reset yes
run absent link cool && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "cool exited $rc"
[ -e "$TMP/mod.log" ] && fail "cool touched the driver"

reset no
run absent link warm >/dev/null || true
[ -e "$TMP/mod.log" ] && fail "insmod on BaseOS"

reset yes
make_fake wpa_cli "$TMP/cli.log" 'sleep 0.3' 'echo "wpa_state=SCANNING"'
t0=$(date +%s)
run absent link join >/dev/null && rc=0 || rc=$?
t1=$(date +%s)
[ "$rc" = 3 ] || fail "slow join exited $rc, want 3"
[ $((t1 - t0)) -le 4 ] || fail "a 2 s join took $((t1 - t0)) s with a slow wpa_cli"

reset yes
run absent update up "$TMP/wifi.conf" && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "update up exited $rc"
grep -q -- "-i wlan0 -c $TMP/run/slotupdate.conf" "$TMP/sup.log" || fail "update up did not start the supplicant on its conf"
head -1 "$TMP/run/slotupdate.conf" | grep -q '^ctrl_interface=' || fail "update conf has no ctrl_interface first"
grep -q 'ssid="Home"' "$TMP/run/slotupdate.conf" || fail "update conf lost the player's network"
grep -q -- '-i wlan0' "$TMP/dhcp.log" || fail "update up asked for no lease"

reset yes
export FAKE_STATE=SCANNING
run absent update up "$TMP/wifi.conf" && rc=0 || rc=$?
unset FAKE_STATE
[ "$rc" = 3 ] || fail "update up with no network exited $rc, want 3"
grep -q terminate "$TMP/cli.log" || fail "update up with no network left the supplicant running"
[ ! -e "$TMP/run/slotupdate.conf" ] || fail "update up with no network left its conf"
[ ! -e "$TMP/dhcp.log" ] || fail "update up asked for a lease with no network"

reset yes
make_fake udhcpc "$TMP/dhcp.log" 'exit 1'
run absent update up "$TMP/wifi.conf" && rc=0 || rc=$?
[ "$rc" = 4 ] || fail "update up with no lease exited $rc, want 4"
grep -q terminate "$TMP/cli.log" || fail "update up with no lease left the supplicant running"
[ ! -e "$TMP/run/slotupdate.conf" ] || fail "update up with no lease left its conf"

reset yes
run absent update up "$TMP/no-such.conf" && rc=0 || rc=$?
[ "$rc" = 3 ] || fail "update up with no conf exited $rc, want 3"
[ ! -e "$TMP/sup.log" ] || fail "update up with no conf started the supplicant"

reset yes
run absent update up "$TMP/wifi.conf" >/dev/null
run absent update down && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "update down exited $rc"
grep -q terminate "$TMP/cli.log" || fail "update down left the supplicant running"
grep -q 'addr flush dev wlan0' "$TMP/ip.log" || fail "update down kept wlan0's address"
[ ! -e "$TMP/run/slotupdate.conf" ] || fail "update down left its conf"

reset yes
run absent update up "$TMP/wifi.conf" >/dev/null
: > "$TMP/cli.log"
run absent link host >/dev/null || true
grep -q terminate "$TMP/cli.log" || fail "a link left the update supplicant on wlan0"
[ ! -e "$TMP/run/slotupdate.conf" ] || fail "a link left the update conf"

reset yes
: > "$TMP/cli.log"
run absent update down && rc=0 || rc=$?
[ "$rc" = 0 ] || fail "update down with nothing up exited $rc"
grep -q terminate "$TMP/cli.log" && fail "update down with nothing up touched wlan0"

reset yes
run absent link sideways 2>/dev/null && rc=0 || rc=$?
[ "$rc" = 2 ] || fail "bad verb exited $rc, want 2"

if [ "$FAILS" -eq 0 ]; then
	echo "slotlink: all passed"
else
	echo "slotlink: $FAILS failed"
	exit 1
fi
