# Home Wi-Fi on Slot

Copy `System/wifi.toml.example` to `System/wifi.toml` on the SD card and enter your network names and passwords. Add one `[[networks]]` block per network, in preferred order. WPA2 personal networks are supported; for an open network use `security = "open"` and omit `password`. Enterprise setup and captive portals are not supported.

In the main menu, change **Home Wi-Fi** to **On**. It defaults to Off and remembers your choice. Slot tries the configured networks in order and keeps a working connection. Association/DHCP run in the background; the shelf never waits for them. A small Wi-Fi symbol beside the battery appears only after the home connection has an IPv4 address. It indicates LAN connectivity, not internet reachability or signal strength.

**Off** disconnects the home network and stops scans/retries without deleting credentials. This ends SSH/SFTP connections using that network. Multiplayer has its own radio lease and remains available. With neither connection needed, Slot blocks Wi-Fi through rfkill while retaining the initialized driver.

Edit the file while Slot is stopped, or toggle Off/On to reload it. Changes are also picked up before reconnect cycles. Invalid replacements leave a running valid configuration intact. Removing the file or clearing its network list and reloading stops home association. Credentials are readable on the SD card; they are never printed in Slot's diagnostics.

Slot uses BaseOS's existing SSH/SFTP service and authentication. It does not enable a disabled SSH server or change passwords. Time synchronization is always enabled through BaseOS's NTP supervisor. It waits for connectivity and does not turn Home Wi-Fi on. The existing `utc_offset_min` in `Config/slot.state` remains the timezone setting, including manual daylight-saving adjustments. No timezone conversion is added to the system/RTC clock.

## Diagnostics and compatibility

With `SLOT_ROOT` set to the card mount, run:

```sh
/bin/sh "$SLOT_ROOT/System/ags-net" service status
/bin/sh "$SLOT_ROOT/System/ags-net" home reload
```

`service stop` tears down Slot-owned network resources before handing the interfaces to another frontend. Slot's running frontend reconnects its service automatically; stop the frontend before handing control to another frontend. Logs are under `/run/slot-services/service.log`. The bundled helper never falls back to the OS's old `ags-net`.

Link uses wlan1 for both host and join. Home uses wlan0. A single radio imposes channel constraints: a new conflicting request fails without intentionally disconnecting the established service. During Link, new home association attempts are deferred. Joining while home is connected requires an advertised two-station, single-channel interface combination; missing/ambiguous capabilities fail closed. `iw` and its libnl libraries ship in `System/slot-net`, invoked through the system loader even on cards without executable bits. wpa_supplicant, wpa_cli, ip, rfkill and udhcpc must be present. Existing Link screens and protocol are unchanged.

The automated suite uses fake networking tools to validate ownership and lifecycle. Real AP+STA and especially STA+STA operation, router channel changes, latency during SFTP, power use, and game RTC behavior during clock correction still require two-device qualification. A capability advertisement is not proof of functioning driver concurrency.

Release packaging includes `slot-services`, `ags-net`, `slot-net` (iw, libraries, licenses and corresponding Debian sources), and this sample. Copy release files over an existing card without deleting `wifi.toml` or `slot.state`. The deployment task never overwrites those user files.
