#!/bin/sh
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
SCRIPT="$HERE/System/launch_frontend.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

setup() {
	rm -rf "$TMP/card" "$TMP/run"
	mkdir -p "$TMP/card/System" "$TMP/run"
	: > "$TMP/card/System/led.sh"
	: > "$TMP/card/System/lid.sh"
}

launch() {
	SLOT_ROOT="$TMP/card" AGS_RUN="$TMP/run" AGS_SPLASH="$TMP/splash" AGS_HOLD=0 \
		AGS_MV="${MV_FAKE:-mv}" AGS_SYNC="$TMP/sync" sh "$SCRIPT" 2>/dev/null || true
}

printf '#!/bin/sh\necho "$*" >> "%s"\n' "$TMP/splash.log" > "$TMP/splash"
chmod 755 "$TMP/splash"
printf '#!/bin/sh\necho sync >> "%s"\n' "$TMP/sync.log" > "$TMP/sync"
chmod 755 "$TMP/sync"

stage() {
	rm -f "$TMP/splash.log"
	mkdir -p "$TMP/card/System.new"
	cp "$SCRIPT" "$TMP/card/System/launch_frontend.sh"
	echo old > "$TMP/card/System/slot"
	cp "$SCRIPT" "$TMP/card/System.new/launch_frontend.sh"
	echo new > "$TMP/card/System.new/slot"
	: > "$TMP/card/System.new/led.sh"
	: > "$TMP/card/System.new/lid.sh"
}

manifest() {
	printf '%s\n' launch_frontend.sh slot led.sh lid.sh "$@" > "$TMP/card/System.new/.manifest"
}

fail() {
	echo "launch: $*" >&2
	exit 1
}

printf '#!/bin/sh\necho "$*" >> "%s"\n' "$TMP/slot.log" > "$TMP/loader"
chmod 755 "$TMP/loader"

logo_boot() {
	rm -f "$TMP/splash.log" "$TMP/slot.log"
	: > "$TMP/splash.log"
	: > "$TMP/slot.log"
	SLOT_ROOT="$TMP/card" AGS_RUN="$TMP/run" AGS_SPLASH="$TMP/splash" AGS_HOLD=0 \
		AGS_LOADER="$TMP/loader" AGS_SYNC="$TMP/sync" sh "$SCRIPT" 2>/dev/null || true
}

setup
mkdir -p "$TMP/card/Config" "$TMP/card/Cache"
echo mine > "$TMP/card/Config/bootlogo.bmp"
logo_boot
grep -q 'INSTALLING BOOT LOGO' "$TMP/splash.log" || fail "a new logo showed no pill"
grep -q -- '--install-bootlogo' "$TMP/slot.log" || fail "a new logo was not installed"
[ "$(grep -n -- '--install-bootlogo' "$TMP/slot.log" | cut -d: -f1)" = 1 ] || fail "slot started before the logo installed"

cp "$TMP/card/Config/bootlogo.bmp" "$TMP/card/Cache/bootlogo.installed"
logo_boot
[ ! -s "$TMP/splash.log" ] || fail "an installed logo showed a pill: $(cat "$TMP/splash.log")"
! grep -q -- '--install-bootlogo' "$TMP/slot.log" || fail "an installed logo was installed again"

rm "$TMP/card/Config/bootlogo.bmp"
echo shipped > "$TMP/card/System/bootlogo.bmp"
logo_boot
grep -q 'INSTALLING BOOT LOGO' "$TMP/splash.log" || fail "going back to the slot logo showed no pill"

setup
logo_boot
[ ! -s "$TMP/splash.log" ] || fail "a card with no logo showed a pill"
! grep -q -- '--install-bootlogo' "$TMP/slot.log" || fail "a card with no logo ran the install"

setup
echo "old run" > "$TMP/card/slot.log"
echo "older run" > "$TMP/card/slot.log.1"
echo "12:00:00 exec" > "$TMP/card/slot-session.log"
echo "alarm" > "$TMP/card/slot-card.log"
echo "boot" > "$TMP/card/ags-shutdown-trace.log"
echo "keys" > "$TMP/card/input-trace.log"
mkdir -p "$TMP/card/crash"
echo "pid=1" > "$TMP/card/crash/cores.log"
launch
for f in slot.log slot.log.1 slot-session.log slot-card.log ags-shutdown-trace.log input-trace.log crash; do
	[ ! -e "$TMP/card/$f" ] || fail "$f was left at the root of the card"
done
[ "$(cat "$TMP/card/Logs/slot.log.1")" = "old run" ] || fail "the last run's log was not kept"
[ "$(cat "$TMP/card/Logs/slot-card.log")" = "alarm" ] || fail "slot-card.log did not move"
[ "$(cat "$TMP/card/Logs/ags-shutdown-trace.log")" = "boot" ] || fail "the shutdown trace did not move"
[ "$(cat "$TMP/card/Logs/input-trace.log")" = "keys" ] || fail "the input trace did not move"
[ "$(cat "$TMP/card/Logs/crash/cores.log")" = "pid=1" ] || fail "crash/ did not move"
[ "$(head -1 "$TMP/card/Logs/slot-session.log")" = "12:00:00 exec" ] || fail "the session log lost its history"
tail -1 "$TMP/card/Logs/slot-session.log" | grep -Eq '^[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9:]{8} exec ' \
	|| fail "the session line has no date: $(tail -1 "$TMP/card/Logs/slot-session.log")"

setup
mkdir -p "$TMP/card/Logs"
echo "this run" > "$TMP/card/Logs/slot.log"
launch
[ "$(cat "$TMP/card/Logs/slot.log.1")" = "this run" ] || fail "slot.log did not rotate"

setup
mkdir -p "$TMP/card/Logs"
head -c 70000 /dev/zero | tr '\0' 'x' > "$TMP/card/Logs/slot-session.log"
launch
[ "$(wc -c < "$TMP/card/Logs/slot-session.log.1")" -eq 70000 ] || fail "a full session log did not rotate"
[ "$(grep -c . "$TMP/card/Logs/slot-session.log")" = "1" ] || fail "the new session log did not start fresh"

setup
mkdir -p "$TMP/card/Logs"
head -c 1000 /dev/zero | tr '\0' 'x' > "$TMP/card/Logs/slot-session.log"
launch
[ ! -e "$TMP/card/Logs/slot-session.log.1" ] || fail "a small session log rotated"

setup
printf 'echo "$*" >> "%s"\n' "$TMP/link.log" > "$TMP/card/System/slotlink.sh"
launch
[ ! -e "$TMP/link.log" ] || fail "tore down a link no earlier run left up"
: > "$TMP/run/slotlink.session"
launch
[ "$(cat "$TMP/link.log" 2>/dev/null)" = "link down" ] || fail "a link left up by a crashed run was not torn down"

setup
stage
echo stale > "$TMP/card/System/stale.txt"
mkdir -p "$TMP/card/System.new/bezels"
echo png > "$TMP/card/System.new/bezels/640x480.png"
manifest bezels/640x480.png
launch
[ "$(cat "$TMP/card/System/slot")" = new ] || fail "a complete staging folder did not replace slot"
[ -f "$TMP/card/System/bezels/640x480.png" ] || fail "a nested file was not moved into System"
[ ! -e "$TMP/card/System/stale.txt" ] || fail "a file the release does not ship was kept"
[ ! -e "$TMP/card/System.new" ] || fail "System.new was left behind"
grep -q "UPDATING SLOT" "$TMP/splash.log" 2>/dev/null || fail "no UPDATING SLOT toast"
grep -q "UPDATE COMPLETE" "$TMP/splash.log" 2>/dev/null || fail "no UPDATE COMPLETE toast"
[ "$(grep -c sync "$TMP/sync.log" 2>/dev/null)" -ge 2 ] || fail "the swap did not sync before and after clearing System.new"

setup
stage
launch
[ "$(cat "$TMP/card/System/slot")" = old ] || fail "a staging folder without a manifest was applied"
[ ! -e "$TMP/card/System.new" ] || fail "a staging folder without a manifest was kept"
[ ! -s "$TMP/splash.log" ] || fail "a discarded staging folder showed a toast"

setup
stage
echo new > "$TMP/card/System/a.txt"
echo new > "$TMP/card/System.new/b.txt"
echo old > "$TMP/card/System/b.txt"
manifest a.txt b.txt
launch
[ "$(cat "$TMP/card/System/a.txt")" = new ] || fail "a file moved before the power cut was lost"
[ "$(cat "$TMP/card/System/b.txt")" = new ] || fail "a half-applied swap did not finish"
[ "$(cat "$TMP/card/System/slot")" = new ] || fail "a half-applied swap did not replace slot"
[ ! -e "$TMP/card/System.new" ] || fail "System.new was left after finishing a swap"

setup
stage
manifest
{
	echo '#!/bin/sh'
	echo "[ -f \"\$SLOT_ROOT/System/launch_frontend.sh\" ] || echo missing >> \"$TMP/mv.log\""
	echo 'exec mv "$@"'
} > "$TMP/mv"
chmod 755 "$TMP/mv"
MV_FAKE="$TMP/mv" launch
[ ! -s "$TMP/mv.log" ] || fail "System/launch_frontend.sh was missing during the swap"
[ "$(cat "$TMP/card/System/slot")" = new ] || fail "the swap through a logging mv did not finish"

setup
stage
manifest
{
	echo '#!/bin/sh'
	echo 'case "$*" in *System.new/slot*) exit 1 ;; esac'
	echo 'exec mv "$@"'
} > "$TMP/mv"
chmod 755 "$TMP/mv"
MV_FAKE="$TMP/mv" launch
grep -q "UPDATE FAILED" "$TMP/splash.log" 2>/dev/null || fail "a failed move showed no UPDATE FAILED toast"
[ -f "$TMP/card/System.new/.manifest" ] || fail "a failed swap threw the staging folder away"
[ "$(cat "$TMP/card/System/slot")" = old ] || fail "a failed swap changed slot"
tail -1 "$TMP/card/Logs/slot-session.log" | grep -q "exec $TMP/card/System/slot" \
	|| fail "a failed swap did not go on to start slot"

setup
rm -f "$TMP/link.log"
printf 'echo "$*" >> "%s"\n' "$TMP/link.log" > "$TMP/card/System/slotlink.sh"
launch
[ ! -e "$TMP/link.log" ] || fail "tore down WiFi no earlier run left up"
: > "$TMP/run/slotwifi.on"
launch
grep -qx "wifi stop" "$TMP/link.log" 2>/dev/null || fail "WiFi left up by a crashed run was not torn down"

echo "launch: all passed"
