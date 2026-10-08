#!/bin/sh
SD="${SLOT_ROOT:-/mnt/sdcard}"
SYS="$SD/System"
RUN="${AGS_RUN:-/run}"
export SLOT_ROOT="$SD"

log() {
	echo "$(date '+%H:%M:%S') $*" >> "$SD/slot-session.log" 2>/dev/null
}

pid="$(cat "$RUN/slot-led.pid" 2>/dev/null)"
if [ -z "$pid" ] || [ ! -d "/proc/$pid" ]; then
	sh "$SYS/led.sh" once
	sh "$SYS/led.sh" loop > /dev/null 2>&1 &
	echo $! > "$RUN/slot-led.pid"
fi

echo "|/bin/sh $SYS/coresave.sh %p %s %e %t" > /proc/sys/kernel/core_pattern 2>/dev/null

for governor in /sys/devices/system/cpu/cpufreq/policy*/scaling_governor; do
	[ -w "$governor" ] || continue
	printf '%s\n' schedutil > "$governor" 2>/dev/null || log "could not set schedutil for $governor"
done

[ -f "$SD/slot.log" ] && mv -f "$SD/slot.log" "$SD/slot.log.1"
log "exec $SYS/slot"

exec /lib/ld-linux-aarch64.so.1 "$SYS/slot" > "$SD/slot.log" 2>&1
