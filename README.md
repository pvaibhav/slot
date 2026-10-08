# slot.

A fork of [slot](https://github.com/BrandonKowalski/slot) by Brandon Kowalski. slot is a
bespoke, Game Boy-centric frontend for the Anbernic RG SP. Brandon wrote slot and its guide.
This fork follows upstream and adds the features listed below.

Has support for GBA, GBC, and GB titles only.

The user guide is upstream's, at [slot-cfw.fyi](https://slot-cfw.fyi). Upstream's release
notes are in the [changelog](CHANGELOG.md).

## What this fork adds

- Home Wi-Fi. Turn it on in the menu and slot connects to the networks listed in
  `Config/wifi.toml`. It also sets the clock from the network.
- Linking over home Wi-Fi. Two handhelds on the same network link through it. Without a
  home network they link directly, as in upstream.
- RetroAchievements. You can earn softcore GBA achievements, online or offline. Put your
  account in `Config/retroachievements.toml`.
- Cart labels. slot downloads missing GBA, GB and GBC labels in the background. It never
  replaces a label you added. Delete a label to download it again.
- Frame blending, and colour calibration for the RG SP and RG34XXSP panels.
- Holding POWER shuts down without showing a menu.

A release includes an `.example` copy of each config file in `Config/`. Copy it, remove
`.example` from the name, and fill it in.

## Versions

A release of this fork uses upstream's version with a suffix. `v1.4.0-pvaibhav.1` is the
first release built on upstream 1.4.0, and `v1.4.0-pvaibhav.2` would be the second.

## AI Disclosure

From upstream:

The Rust frontend was put together by Claude Opus. I reviewed everything that was
produced. All documentation is 100% free-range, meatbag prose.

The project is extremely low stakes. I wanted a bespoke frontend for my RG SP and thought
that something that evokes the feeling of using my GBA SP as a kid would be pretty neat.

Use it, don't use it, I don't care.

Figured I should share the end result of all the wasted water. ✌🏻

This fork's additions were written with AI assistance too.
