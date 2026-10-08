# slot.

A fork of [slot](https://github.com/BrandonKowalski/slot) by Brandon Kowalski: a bespoke,
Game Boy-centric frontend for the Anbernic RG SP. slot itself, its design and its guide are
his work; this fork follows upstream and adds a few things on top.

Has support for GBA, GBC, and GB titles only.

The full user guide is upstream's, at [slot-cfw.fyi](https://slot-cfw.fyi), and its release
notes are in the [changelog](CHANGELOG.md).

## What this fork adds

- **Home Wi-Fi.** List your networks in `Config/wifi.toml` and turn Home Wi-Fi on in the menu.
  The clock sets itself from the network.
- **Link over your home network.** Two handhelds on the same Wi-Fi link through it, and stay
  connected to it while they play. Without a home network they link directly, as before.
- **RetroAchievements.** Earn softcore GBA achievements, online or offline, with a short
  notification when one unlocks. Fill in `Config/retroachievements.toml`.
- **Automatic cart labels.** Missing GBA, GB and GBC labels are fetched in the background and
  appear as they arrive. Your own labels are never replaced; delete one to have it fetched again.
- **A closer picture.** Frame blending like the original screen, and colour calibrated for the
  RG SP's panel.
- **POWER means off.** Holding POWER shuts down straight away, with no menu to answer.

Both config files ship as `.example` files in `Config/`. Copy one, drop the `.example`, and
fill it in.

## AI Disclosure

From upstream:

The Rust frontend was put together by Claude Opus. I reviewed everything that was
produced. All documentation is 100% free-range, meatbag prose.

The project is extremely low stakes. I wanted a bespoke frontend for my RG SP and thought
that something that evokes the feeling of using my GBA SP as a kid would be pretty neat.

Use it, don't use it, I don't care.

Figured I should share the end result of all the wasted water. ✌🏻

This fork's additions were written with AI assistance too.
