# v1.16.0-pvaibhav.2

- **armGBA is the new default core for GBA games.** It keeps full speed without the `performance` CPU governor that mGBA needs. Games you already set to a core keep it.
- Save states stay with the core that made them. To access a game's mGBA states, press Start on its cart and move the chip to mGBA. Battery saves work on every core. When you switch cores, Slot sets aside any resume for the selected core that predates your battery save.
- The core picker has three sockets. GB and GBC games can use armGBA too; their default is unchanged.
- Cheats work on armGBA as well as mGBA.
- armGBA can link GBA games over the cable. A friend who joins from mGBA or gpSP switches to armGBA for the link. This is new and was tested only in simulation. GB, GBC and Wireless Adapter links still need mGBA or gpSP.
- On armGBA, original Game Boy games show in grayscale. Use an Authentic shader for the classic look.
- Authentic honors the Reflection setting again. Frame Blending works with Authentic, and Authentic defaults to GPU Ghosting.

Update from **System > Check for Updates**, or copy the contents of `slot-v1.16.0-pvaibhav.2/` from the release ZIP onto SD2.

`Cheats.pack` and `Artwork.pack` are unchanged. If you’re installing them for the first time, put the attached packs in `Cheats/` and `Labels/`.

See the [upstream changelog](https://github.com/slot-cfw/slot/blob/v1.16.0/CHANGELOG.md) for all upstream changes.
