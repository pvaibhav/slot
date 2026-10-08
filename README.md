# slot.

A fork of [slot](https://github.com/BrandonKowalski/slot) by Brandon Kowalski. slot is a
bespoke, Game Boy-centric frontend for the Anbernic RG SP. Brandon wrote slot and its guide.
This fork follows upstream and adds the features listed below.

Has support for GBA, GBC, and GB titles only.

The user guide is upstream's, at [slot-cfw.fyi](https://slot-cfw.fyi). Upstream's release
notes are in the [changelog](CHANGELOG.md).

## What this fork adds

- **LCD ghosting / interframe blending** is enabled by default. It's subtle.
- **Colour calibration** for the RG SP and RG34XXSP panels, based on an accurate 3x1D LUT
  measured by me. Both white point and gamma curve are corrected. Currently no other firmware
  has this!
- Holding POWER **shuts down** directly, without showing a menu. I did not find value in a
  restart option.
- **Home Wi-Fi.** Slot can now connect to your home Wi-Fi. Just set up your Wi-Fi credentials
  in `Config/wifi.toml` and then turn HOME WI-FI on from the menu. Everything happens
  automatically from then onwards. Most of the features below depend on this.
- **Hassle-free cart label scraping.** Just throw your ROMs on the SD card, and slot downloads
  missing labels in the background. It will not replace a label you added yourself, though.
  No config is needed for this feature.
- **RetroAchievements.** You can earn softcore GBA achievements, online or offline. Put your
  account in `Config/retroachievements.toml` for this to work. All your games' achievements
  will be cached if Wi-Fi is on, so once the sync is done, you can go out and play without
  losing your achievements. They'll sync automatically when you're back home. When you're
  playing at home with Wi-Fi on, your achievements will sync in real time with rich presence.
- **Linking over home Wi-Fi.** When at home and connected to Wi-Fi, two handhelds can link
  using that instead of having to create ad hoc networks. Outside of home, you can continue
  using slot's normal method. This works seamlessly, no config needed.

## How to configure

The release includes an `.example` copy of each config file in `Config/`. Remove `.example`
from the name, and fill in your details. You only need to configure your Wi-Fi and
RetroAchievements credentials. Everything else is automatic.

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
