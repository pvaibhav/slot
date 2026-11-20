# Unreleased

- **armGBA is the new default core for GBA games.** It keeps full speed without the `performance` CPU governor that mGBA needs. Games you already set to a core keep it.
- Save states stay with the core that made them. To get a game's mGBA states back, press Start on its cart and move the chip to mGBA. Battery saves work on every core.
- The core picker has three sockets. GB and GBC games can use armGBA too; their default is unchanged.
- Cheats work on armGBA as well as mGBA.
- armGBA can link GBA games over the cable. A friend who joins from mGBA or gpSP switches to armGBA for the link. This is new and was tested only in simulation. GB, GBC and Wireless Adapter links still need mGBA or gpSP.
- On armGBA, Game Boy palettes have no effect and original Game Boy games show in grayscale; use an Authentic shader for the classic look.

# v1.16.0-pvaibhav.1

- Updated to upstream v1.16.0: handwritten titles on carts without label art, Kid Mode link invites, player nicknames, and USB-C audio support.
- GPU Frame Blending now uses upstream’s implementation. Authentic keeps its LCD models and reflection setting.
- Keeps this fork’s automatic 120 Hz, CPU scaling, panel calibration, RetroAchievements, cheats, and offline cart labels.
- To turn turbo off, clear Turbo A and Turbo B in **Settings > Controls**; the separate Turbo Buttons setting is gone.

Update from **System > Check for Updates**, or copy the contents of `slot-v1.16.0-pvaibhav.1/` from the release ZIP onto SD2.

`Cheats.pack` and `Artwork.pack` are unchanged. Existing copies need no update; new users can put the attached packs in `Cheats/` and `Labels/`. Cheats require mGBA.

See the [upstream changelog](https://github.com/slot-cfw/slot/blob/v1.16.0/CHANGELOG.md) for all upstream changes.
