#!/bin/sh
SD="${SLOT_ROOT:-/mnt/sdcard}"
SYS="$SD/System"
RUN="${AGS_RUN:-/run}"
LOGS="$SD/Logs"
SESSION_MAX=65536
NEW="$SD/System.new"
SPLASH="${AGS_SPLASH:-/usr/bin/baseos-splash}"
MV="${AGS_MV:-mv}"
HOLD="${AGS_HOLD:-2}"
SYNC="${AGS_SYNC:-sync}"
LOADER="${AGS_LOADER:-/lib/ld-linux-aarch64.so.1}"
export SLOT_ROOT="$SD"
export SLOT_RUN="$RUN"

mkdir -p "$LOGS" 2>/dev/null
for f in slot.log slot.log.1 slot-session.log slot-session.log.1 slot-card.log ags-shutdown-trace.log input-trace.log; do
	[ -f "$SD/$f" ] && mv -f "$SD/$f" "$LOGS/$f"
done
[ -d "$SD/crash" ] && [ ! -e "$LOGS/crash" ] && mv "$SD/crash" "$LOGS/crash"

[ $(wc -c < "$LOGS/slot-session.log" 2>/dev/null || echo 0) -gt "$SESSION_MAX" ] \
	&& mv -f "$LOGS/slot-session.log" "$LOGS/slot-session.log.1"

log() {
	echo "$(date '+%F %T') $*" >> "$LOGS/slot-session.log" 2>/dev/null
}

now() {
	cut -d' ' -f1 /proc/uptime 2>/dev/null || date +%s
}

shown=
say() {
	if [ -n "$shown" ]; then
		left="$(awk -v a="$shown" -v b="$(now)" -v h="$HOLD" \
			'BEGIN { l = h - (b - a); if (l > 0) printf "%.2f", l; else print 0 }')"
		[ "$left" = 0 ] || sleep "$left"
	fi
	"$SPLASH" --important -1 "$1" 2>/dev/null
	shown="$(now)"
}

put() {
	[ -e "$NEW/$1" ] || return 0
	mkdir -p "$(dirname "$SYS/$1")" && $MV -f "$NEW/$1" "$SYS/$1"
}

apply_update() {
	[ -d "$NEW" ] || return 0
	if [ ! -f "$NEW/.manifest" ]; then
		log "discarded an unfinished update"
		rm -rf "$NEW"
		return 0
	fi
	log "applying update"
	say "UPDATING SLOT"
	while IFS= read -r f; do
		case "$f" in
		"" | slot | launch_frontend.sh) continue ;;
		esac
		put "$f" || { log "update: could not move $f"; say "UPDATE FAILED"; return 1; }
	done < "$NEW/.manifest"
	for f in slot launch_frontend.sh; do
		put "$f" || { log "update: could not move $f"; say "UPDATE FAILED"; return 1; }
	done
	$SYNC
	(cd "$SYS" && find . -type f) | sed 's|^\./||' | while IFS= read -r f; do
		grep -qxF "$f" "$NEW/.manifest" || rm -f "$SYS/$f"
	done
	find "$SYS" -mindepth 1 -type d -empty -delete 2>/dev/null
	rm -rf "$NEW"
	$SYNC
	log "update applied"
	say "UPDATE COMPLETE"
	exec /bin/sh "$SYS/launch_frontend.sh"
}

apply_update

pid="$(cat "$RUN/slot-led.pid" 2>/dev/null)"
if [ -z "$pid" ] || [ ! -d "/proc/$pid" ]; then
	sh "$SYS/led.sh" once
	sh "$SYS/led.sh" loop > /dev/null 2>&1 &
	echo $! > "$RUN/slot-led.pid"
fi

pid="$(cat "$RUN/slot-lid.pid" 2>/dev/null)"
if [ -z "$pid" ] || [ ! -d "/proc/$pid" ]; then
	sh "$SYS/lid.sh" loop > /dev/null 2>&1 &
	echo $! > "$RUN/slot-lid.pid"
fi

echo "|/bin/sh $SYS/coresave.sh %p %s %e %t" > /proc/sys/kernel/core_pattern 2>/dev/null

[ -f "$RUN/slotlink.session" ] && sh "$SYS/slotlink.sh" link down > /dev/null 2>&1
[ -f "$RUN/slotwifi.on" ] && sh "$SYS/slotlink.sh" wifi stop > /dev/null 2>&1

logo="$SD/Config/bootlogo.bmp"
[ -f "$logo" ] || logo="$SYS/bootlogo.bmp"
if [ -f "$logo" ] && ! cmp -s "$logo" "$SD/Cache/bootlogo.installed"; then
	log "installing boot logo"
	say "INSTALLING BOOT LOGO"
	"$LOADER" "$SYS/slot" --install-bootlogo >> "$LOGS/slot-session.log" 2>&1
fi

[ -f "$LOGS/slot.log" ] && mv -f "$LOGS/slot.log" "$LOGS/slot.log.1"
log "exec $SYS/slot"

exec "$LOADER" "$SYS/slot" > "$LOGS/slot.log" 2>&1
