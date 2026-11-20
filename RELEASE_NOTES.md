# Unreleased

- **armGBA is the new default core for GBA games.** It keeps full speed without the `performance` CPU governor that mGBA needs. Games you already set to a core keep it.
- Save states stay with the core that made them. To get a game's mGBA states back, press Start on its cart and move the chip to mGBA. Battery saves work on every core.
- The core picker has three sockets. GB and GBC games can use armGBA too; their default is unchanged.
- Cheats work on armGBA as well as mGBA.
- Two devices on armGBA can link GBA games over the cable. This is new and was tested only in simulation. GB, GBC and Wireless Adapter links still need mGBA or gpSP, and both devices must use the same core.
- On armGBA, Game Boy palettes have no effect and original Game Boy games show in grayscale; use an Authentic shader for the classic look.

# v1.15.3-pvaibhav.1

- Updated to upstream v1.15.3: separate **Resume on Insert** and **Resume on Power On** settings, battery icon/percentage choices, and a reorganized settings menu.
- Wallpapers gain a live tint preview and **Per Shelf** defaults. Bundled wallpapers are now ordinary files you can remove.
- Keeps this fork’s Authentic shaders, GPU frame blending, panel calibration, native game pacing, RetroAchievements, cheats, and offline cart labels.

Install from **Network > Check for Updates** on the previous release (now **System > Check for Updates**), or copy the contents of `slot-v1.15.3-pvaibhav.1/` from the release ZIP onto SD2.

`Cheats.pack` and `Artwork.pack` are unchanged. Existing copies need no update; new users can put the attached packs in `Cheats/` and `Labels/`. Cheats require mGBA.

See the [upstream changelog](https://github.com/slot-cfw/slot/blob/v1.15.3/CHANGELOG.md) for all upstream changes.
