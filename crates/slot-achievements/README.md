# RetroAchievements

slot can earn **softcore GBA achievements**, online or offline, with a short in-game
1.8-second notification showing the achievement's cached RA badge, name and points, accompanied by a quiet
240 ms chime that follows the game volume and mute setting. There is no achievement browser or
account menu. Save states, resume, rewind, and fast-forward remain available.

The notification slides and fades in, gives the badge a small pop, then fades away.
The total duration includes both transitions. A small trophy is used only when a badge is
missing or invalid; showing an unlock never waits for a download.

Copy the bundled `System/retroachievements.toml.example` to `System/retroachievements.toml`
on the card and fill in your account:

```toml
enabled = true
username = "your-ra-username"
password = "your-ra-password"
```

Restart slot. A successful login remembers the account's login token privately under
`Saves/RetroAchievements/`; you can then remove the `password` line from the config.
Alternatively, set `token = "your-login-token"` instead of `password`. This is an
**emulator login token**, such as one obtained by NextUI, not a RetroAchievements Web API
key. An explicitly configured token takes precedence over a password or remembered token.
Account and configuration changes take effect on restart. Omit the file or set
`enabled = false` to disable the feature. TOML comments and literal strings are supported;
for example, single quotes preserve backslashes in a password without escaping them.

For home networking, configure `System/wifi.toml` from its bundled example and turn
**Home Wi-Fi** **On** once in the main menu. The toggle is remembered across reboots.
Wi-Fi connection, network time sync, RA login, and library preparation then run automatically.
When the Wi-Fi worker reports a new connection, RA retries immediately instead of waiting
for its normal network backoff. Releases and deployment copy only the example files;
existing credentials and caches are preserved.

Treat the config and cached login token as credentials. slot does not print them in logs.
It uses verified HTTPS with bundled trusted roots; it needs neither curl nor a system CA
bundle on the handheld. The device clock must be reasonably accurate for TLS and offline
unlock timestamps.

## Automatic offline preparation

While slot is running, it automatically prepares every `.gba` file in `Games/GBA` whenever
the server is reachable. You do not have to open each game first. The current game has
priority; other ROMs are prepared gradually in the background. New/changed ROMs are
discovered every five minutes, and cached definitions and server unlocks refresh weekly.
Library preparation does not start play sessions on the server.
Unlocked badge images are also cached in the background, including for games you have not
opened. They remain available offline. Missing or damaged images are retried independently;
badge downloads take lower priority than unlock uploads and game setup. Older caches are
refreshed once to add badge metadata.
The shelf's bottom bar has a small sync icon just left of the clock. It rotates while
preparing or syncing, shows badge-cache percentage once ROM discovery establishes the total,
disappears when caught up, and shows an amber dot when queued work is waiting for
internet or when account/data/storage needs attention. It is absent when disabled. Routine
status never interrupts gameplay with a banner; only earned achievements show a banner.
Background diagnostics go to slot's log.

Each ROM needs its initial download once. Leave slot running online long enough to prepare
the library before taking a newly populated card offline. Unknown ROM hashes cannot earn
achievements; use a ROM version supported by RetroAchievements. Cached games start tracking
without waiting for a network timeout. Initial preparation is asynchronous; events before
tracking starts cannot be recovered.

Unlocks are saved atomically before the trophy notification appears. They survive
ejects, shutdowns, and restarts. Automatic sync covers all games, including while the shelf
is open. Network failures back off from 30 seconds to five minutes and retry automatically;
no manual sync is needed. Only server-confirmed awards leave the pending state. Confirmed
records are retained so an offline restart does not award the same achievement again.

The cache and unlock ledger are separated by account. Keep `Saves/RetroAchievements` when
backing up or moving the card: deleting it removes unsynced unlocks and offline preparation.
Do not edit `unlocks.json`. A damaged ledger is reported and preserved, not silently replaced.

## Gameplay and networking

Evaluation, persistence, ROM hashing, library scanning, HTTPS, and banner rasterization run
on background threads. The emulator copies up to 352 KiB of GBA RAM after each emulated
frame into a bounded queue (including fast-forward frames); it never waits for the worker.
No emulator pointers cross threads. If evaluation falls behind, slot logs an interruption
and resets achievement hit/delta tracking across the missing frames, avoiding false awards.
State loads and rewind also reset partial achievement progress.

Both mGBA and gpSP expose the required RAM. gpSP's network link can continue tracking from
the cache when its softAP has no internet. mGBA's two-console cable mode is excluded in this
first version because its memory belongs to two players. Leaderboards, hardcore mode,
and achievement browsing are not included.

The Wi-Fi configuration UI is independent: this feature uses ordinary HTTPS whenever the OS
has a route to the internet. It does not configure radios, interfaces, softAP, or Wi-Fi.

## Implementation and verification

`slot-achievements` vendors the official rcheevos 12.2.1 evaluator at the same revision used
by NextUI. The REST protocol and GBA memory layout follow that revision's API helpers and
console definitions. Account authentication uses `login2`; preparation uses `gameid`,
`patch`, and `unlocks`; play uses `startsession` and periodic `ping`. Deferred
`awardachievement` requests include the original unlock age and the corresponding signature.

The focused tests use a fake server and real rcheevos conditions to cover offline earning,
restart/replay, account isolation, automatic preparation, failed writes, rejected awards,
dropped frames, and rewind. `slot-retro` also has a real-core memory mapping test, run when
host mGBA/gpSP cores are available in `vendor/`.

```sh
cargo test -p slot-achievements -p slot-retro
cargo clippy --workspace --all-targets -- -D warnings
task build:device
```

A real account and an on-device offline/online session are still needed to verify live
server acceptance and handheld performance. The tests never send an achievement to a real
account.

Rich presence scripts are cached with game data and evaluated on the achievement
worker. Starting an online game immediately publishes a playing status; heartbeats
refresh it every two minutes using the game's rich presence, with a title fallback.
Presence is live-only: offline activity is not replayed as a current playing status.
