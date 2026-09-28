# slot.

A bespoke, Game Boy-centric frontend for the Anbernic RG SP.

Has support for GBA, GBC, and GB titles only.

A full user guide can be found at [slot-cfw.fyi](https://slot-cfw.fyi).

Release notes can be found in the [changelog](CHANGELOG.md).

## Cartridge labels

Slot fetches missing cartridge artwork from LaunchBox in the background, prepares
196×86 PNGs in `Labels/GBA`, and updates the UI as each label arrives. Existing
labels are preserved. Unavailable artwork keeps the generated text label.

Downloads have a 15-second timeout per request and at most three attempts per
ROM per launch, with 30-second and two-minute retry delays. Three consecutive
network failures stop the worker until the next launch. Missing or ambiguous
matches and unsupported images are skipped for that launch; they do not block
other games. The worker never blocks the UI or shutdown.

The [standalone GBA label downloader](tools/gba-labels/README.md) is also available
for preparing labels on a computer, with previews and an optional copy step.

## RetroAchievements

Optional [RetroAchievements](crates/slot-achievements/README.md) support uses TOML configuration,
automatic offline preparation and synchronization, and brief achievement notifications.

## AI Disclosure

The Rust frontend was put together by Claude Opus. I reviewed everything that was
produced. All documentation is 100% free-range, meatbag prose.

The project is extremely low stakes. I wanted a bespoke frontend for my RG SP and thought
that something that evokes the feeling of using my GBA SP as a kid would be pretty neat.

Use it, don't use it, I don't care.

Figured I should share the end result of all the wasted water. ✌🏻