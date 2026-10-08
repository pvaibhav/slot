# Home Wi-Fi on Slot

Copy `Config/wifi.toml.example` to `Config/wifi.toml` on the SD card and enter your network names and passwords. Add one `[[networks]]` block per network, in preferred order. WPA2 personal networks are supported; for an open network use `security = "open"` and omit `password`. Enterprise setup and captive portals are not supported.

In the main menu, change **Home Wi-Fi** to **On**. It defaults to Off and remembers your choice. Slot tries the configured networks in order and keeps a working connection. Association/DHCP run in the background; the shelf never waits for them. A small Wi-Fi symbol beside the battery appears only after the home connection has an IPv4 address. It indicates LAN connectivity, not internet reachability or signal strength.

**Off** disconnects the home network and stops scans/retries without deleting credentials. This ends SSH/SFTP connections using that network. Multiplayer works with Home Wi-Fi On or Off (see below). With neither connection needed, Slot blocks Wi-Fi through rfkill while retaining the initialized driver.

Edit the file while Slot is stopped, or toggle Off/On to reload it. Changes are also picked up before reconnect cycles. Invalid replacements leave a running valid configuration intact. Removing the file or clearing its network list and reloading stops home association. Credentials are readable on the SD card; they are never printed in Slot's diagnostics.

Slot uses BaseOS's existing SSH/SFTP service and authentication. It does not enable a disabled SSH server or change passwords. Time synchronization is always enabled through BaseOS's NTP supervisor. It waits for connectivity and does not turn Home Wi-Fi on. The existing `utc_offset_min` in `Config/slot.state` remains the timezone setting, including manual daylight-saving adjustments. No timezone conversion is added to the system/RTC clock.

## Diagnostics and compatibility

With `SLOT_ROOT` set to the card mount, run:

```sh
/lib/ld-linux-aarch64.so.1 "$SLOT_ROOT/System/slot-services" service status
/lib/ld-linux-aarch64.so.1 "$SLOT_ROOT/System/slot-services" home reload
```

`service stop` tears down Slot-owned network resources before handing the interfaces to another frontend. Slot's running frontend reconnects its service automatically; stop the frontend before handing control to another frontend. Logs are under `/run/slot-services/service.log`.

## Multiplayer

A link uses whichever network the device is already on:

- **Home Wi-Fi connected (a "HOME WI-FI" plate on the link screen).** Both handhelds meet over the home network. Nothing is brought up: no access point, no channel to share, so it works on any channel including DFS ones, and Home Wi-Fi stays connected throughout. The host announces itself as `<name>._slotlink._tcp.local` (mDNS, UDP 5353) with the cart's header code, and the joiner asks for hosts running the same game and connects to whoever answers on TCP :7211 (or `SLOT_LINK_PORT`). A host only accepts a peer that opens with its game's greeting. BaseOS's own Avahi is not needed and does not conflict: it only publishes `<hostname>.local`, and Slot's announcements work even with `mdns=false`.
- **No home connection (a "DIRECT LINK" plate).** As before: the host brings up a private access point on wlan1 (`slotlink`, 10.42.0.1) and the joiner joins it (10.42.0.2), TCP :7211. If Home Wi-Fi is On but has not connected (out of range, still scanning), Slot pauses it for the session and resumes it afterwards.

Both players need to be in the same mode: one handheld on the home network and the other with Wi-Fi off cannot see each other, and the plates show which each is using. The home network must let devices talk to each other: an access point with client isolation, or a guest network, blocks the LAN mode and the joiner ends on "Nobody arrived". Turn Home Wi-Fi Off on both to use the direct link instead.

`slot-services link lan` asks the service whether the home network is up and prints its address (`0 192.168.1.24`), or exits 1 with `NO_HOME_LAN`; it takes no lease. `slot-services link host|join` refuse with `HOME_CONNECTED` while Home Wi-Fi is connected, since an access point beside a live home connection would fight it for the one channel.

`iw` and its libnl libraries ship in `System/slot-net`, invoked through the system loader even on cards without executable bits. wpa_supplicant, wpa_cli, ip, rfkill and udhcpc must be present. The 8821cs driver prints no `channel` line in `iw dev <if> info`, so the channel is taken from `wpa_cli status`.

The automated suite uses fake networking tools to validate ownership and lifecycle. Router channel changes, latency during SFTP, power use, mDNS behaviour under Wi-Fi power save, and game RTC behavior during clock correction still require two-device qualification.

Release packaging includes `slot-services`, `slot-net` (iw and its libraries, with their licenses and Debian sources in `System/licenses`), and `Config/wifi.toml.example`. Copy release files over an existing card without deleting `wifi.toml` or `slot.state`. The deployment task never overwrites those user files.
