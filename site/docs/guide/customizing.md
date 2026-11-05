# Customization

## Cart labels

Drop a `.png` into the `Labels/{Platform}` folder with the same name as the ROM.

e.g. `Labels/GBA/Metroid Fusion.png` goes with `Games/GBA/Metroid Fusion.gba`.

| Cart           | Shape  | Suggested size |
|----------------|--------|----------------|
| GBA            | 1.94:1 | 1280×661       |
| Game Boy       | 1.12:1 | 1024×914       |
| Game Boy Color | 1.12:1 | 1024×914       |

slot will cache labels on the next boot after adding labels.

You don't have to make these by hand and can use the [Cart Studio](https://studio.slot-cfw.fyi) to fetch and apply
labels.

## Cart Studio on the device

Press <kbd>SELECT</kbd> on the shelf to open Cart Studio for the cart that is currently selected.

You can print a new label (requires Wi-Fi) and change shell color / finish.

## Cart shells

You can also customize the color and appearance of the cart shell.

To pick your own, add a line to `Labels/cart_shell.txt`:

```
Metroid Fusion = auto 6a3fa0 solid
```

That's the game's file name, then three words:

1. **Shape**: `auto`, `notched` or `rounded`. Only matters for Game Boy carts. `auto` uses the notched cart for original
   Game Boy games and the rounded one for Color-only games.
2. **Color**: six hex digits, no `#`.
3. **Finish**: `solid`, `clear` or `glitter`.

Keep all three options lowercase. A line slot can't read is skipped.

## Wallpapers

Drop a `.png` into `Wallpapers/` and pick it under Wallpaper in Settings → General.

Random picks one at boot.

Shelf changes the wallpaper as you move between shelves. Name a file after each shelf:

| Shelf                  | File                                |
|------------------------|-------------------------------------|
| Game Boy               | `GB.png`                            |
| Game Boy Color         | `GBC.png`                           |
| Game Boy Advance       | `GBA.png`                           |
| Game Boy Advance Video | `GBAV.png`, otherwise `GBA.png`     |
| Favorites              | `Favorites.png`                     |
| Recents                | `Recents.png`                       |

A shelf without a file shows the default wallpaper.

| Device                                  | Suggested Size |
|-----------------------------------------|----------------|
| RG SP, RG34XX, RG34XXSP                 | 720×480        |
| RG35XXSP, RG35XX Plus, RG35XX-H, RG28XX | 640×480        |

Other sizes are scaled to fill the screen and cropped from the center.

If your wallpaper is too bright, the Wallpaper Tint setting can darken it a bit to make everything easier to read.

## Bezels

Drop a `.png` into `Bezels/GBA/`, `Bezels/GBC/` or `Bezels/GB/` and pick it in the Bezel row of the
`Settings → Screen` menu for the system.

The bezel is drawn over the game, so it needs a transparent hole where the game goes.

Where this hole is located in the bezel image depends on the Scale you have selected.

A bezel without a suffix will appear in the list regardless of the currently selected Scale. If the bezel filename
ends in one of the following suffixes, it will only appear when that Scale is selected.

| Scale     | Suffix      | Where the game goes                                |
|-----------|-------------|----------------------------------------------------|
| SP Screen | `_spscreen` | In the middle, like a GBA SP shows Game Boy |
| Integer   | `_integer`  | The biggest whole-number size, in the middle       |
| Fit       | `_fit`      | As big as the whole picture fits                   |

!!! info "Why would I use SP Screen over Integer?"

    SP Screen allows you to use a bottom bezel with Game Boy and Game Boy Color games.

    Paired with the bundled AGS-102 bezel, it makes your 4:3 device look like a GBA SP.

## Bezel templates

Each template is the screen in grey with a hole where the game goes. Paint over the grey and keep the hole clear.

| Screen        | Game Boy and Game Boy Color                                                                                                                           | GBA                                                                                              |
|---------------|-------------------------------------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------|
| 640×480 (4:3) | [SP Screen](../assets/bezels/640x480-gb-spscreen.png), [Integer](../assets/bezels/640x480-gb-integer.png), [Fit](../assets/bezels/640x480-gb-fit.png) | [Integer](../assets/bezels/640x480-gba-integer.png), [Fit](../assets/bezels/640x480-gba-fit.png) |
| 720×480 (3:2) | [Integer](../assets/bezels/720x480-gb-integer.png), [Fit](../assets/bezels/720x480-gb-fit.png)                                                        | None                                                                                             |

## Boot logo

!!! info "Skip the BMP homework"
    Turn any image into a boot logo that's the right size and format for your device.

    [Open Boot Logo Studio](../bootlogo-studio.md){ .btn }

Put a `.bmp` at `Config/bootlogo.bmp`, or upload one on Gamebit's Boot Logo page.

slot will install it on the next boot, so you'll see it on the second power-on after adding it.

slot copies the file as it is, without resizing it.

Boot logos must be a 24-bit, uncompressed `.bmp` at your device's screen size:

| Device                                                                         | Size                                 |
|--------------------------------------------------------------------------------|--------------------------------------|
| RG35XX Plus, RG35XX 2024, RG35XX H, RG35XX Pro, RG35XX SP, RG40XX H, RG40XX V | 640×480                              |
| RG SP, RG34XX, RG34XX SP                                                       | 720×480                              |
| RG CubeXX                                                                      | 720×720                              |
| RG28XX                                                                         | 480×640, turned 90° counterclockwise |

A smaller image is centred on black. A larger one, a 32-bit one or a compressed one won't show, and you'll get no logo at boot.

Delete `Config/bootlogo.bmp` to go back to the slot logo.

## BIOS

slot doesn't need any BIOS files. If you have your own, drop them into `BIOS/` and you'll get the real boot animation
when a game starts.

- `gba_bios.bin`
- `gbc_bios.bin`
- `gb_bios.bin`

If you link with mGBA, both devices need the same BIOS setup. See [Linking Up](linking.md).