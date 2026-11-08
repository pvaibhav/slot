# v1.15.2-pvaibhav.1

Updated to upstream **v1.15.2**, including everything from 1.15.0 and 1.15.1.

- **Collections** group games across systems. Hold Y on a game to add it to a collection.
- **Kid Mode** limits slot to choosing and playing games.
- **Auto Load State** gains a **Power On** option: inserted carts start fresh, while powering on resumes your game.
- Upstream fixes include local-time cartridge clocks, Gambatte rumble, Pikachu's voice in Pokémon Yellow, smoother LCD rendering on 4:3 screens, and Cart Studio and collection display fixes.
- BaseOS network time and `slot.local` now use upstream's implementations, which include the changes previously shipped by this fork.

This fork keeps its Authentic LCD shaders, panel calibration, automatic 120 Hz with native game pacing, CPU scaling, RetroAchievements, cheats, offline artwork packs, and Wi-Fi during play. Updates continue to come from this fork.

Update from **Network > Check for Updates**, or copy the contents of `slot-v1.15.2-pvaibhav.1/` from the release ZIP onto SD2. Keep your existing `Cheats.pack` and `Artwork.pack`. Cheats require mGBA.

See the [upstream changelog](https://github.com/pvaibhav/slot/blob/v1.15.2-pvaibhav.1/CHANGELOG.md) for the full list of changes.
