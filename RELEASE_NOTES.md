# v1.14.0-pvaibhav.1

This release builds on upstream **v1.13.0** and **v1.14.0**. From upstream you get the Network menu for Wi-Fi, Gamebit for managing the card from a browser, the on-device Cart Studio with Fetch Missing Art, over-the-air updates, the Shelf wallpaper, and Gambatte as a new Game Boy core. Upstream's [changelog](https://github.com/pvaibhav/slot/blob/v1.14.0-pvaibhav.1/CHANGELOG.md) lists everything.

## Changes in this fork

- **Wi-Fi now uses upstream's Network menu.** Join your network again from Network in the shelf menu. On first start, slot deletes the old `Config/wifi.toml` and the `slot-services` and `slot-net` files it no longer uses.
- **Wi-Fi stays connected while you play.** If Wi-Fi is connected when you put a cart in, it stays connected, so RetroAchievements unlocks and rich presence reach the site as you play. slot never starts a connection during a game, because a scan can stall frames. Achievements earned offline sync as soon as Wi-Fi connects again.
- **Online label scraping is now upstream's.** Fetch Missing Art under Network replaces this fork's LaunchBox downloads. `Artwork.pack` in `Labels/` still labels your carts with no network.
- **Linking over home Wi-Fi is removed.** Links use upstream's direct link, which needs no network. Wi-Fi is off during a link, and achievements you earn then sync after it ends.
- **Set Automatically uses BaseOS's network time.** The row in Date & Time turns BaseOS's `ntpd` on or off. slot no longer asks a time server itself, and Date & Time shows the choice BaseOS has.
- **Gamebit answers at `slot.local`, and BaseOS still answers at your device's own name.** If another device already has `slot.local`, slot takes `slot-2.local` and prints that name on its receipt. `mdns=false` in BaseOS's settings turns both off.
- **Check for Updates installs this fork's releases, never upstream's.** This is the first release the updater can find, so the next fork release can be installed from Network > Check for Updates.

Cheats still need the mGBA core. New cards start Game Boy games on Gambatte, which has no cheats yet. Press START on a Game Boy cart to switch it to mGBA. Existing cards stay on mGBA.

## Install or update

Unzip `slot-v1.14.0-pvaibhav.1-h700.zip`, then copy the contents of `slot-v1.14.0-pvaibhav.1/` onto SD2. The copy keeps your games, saves, settings, and credentials. Join your Wi-Fi network again from Network in the shelf menu.

`Cheats.pack` and `Artwork.pack` are unchanged. Keep your existing copies, or put them in `Cheats/` and `Labels/` respectively.

Cheats are from the [libretro database](https://github.com/libretro/libretro-database) and its contributors, under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). Cart artwork is from the [LaunchBox Games Database](https://gamesdb.launchbox-app.com) and belongs to the respective publishers; the artwork pack is for personal use.

Emulation by mGBA (MPL-2.0), gpSP (GPL-2.0) and Gambatte (GPL-2.0). Licences, patches, and the gpSP and Gambatte sources are in `System/licenses/`.
