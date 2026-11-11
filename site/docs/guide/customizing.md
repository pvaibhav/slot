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

Drop a `.png` into `Wallpapers/` and pick it under Wallpaper in Settings → Shelf.

slot puts its own wallpaper in the folder as `Slot.png`, with a blurred version as `Slot Blur.png`. They're
normal files: delete them and they stay gone. None leaves the shelf on a plain background.

Random picks one of your own wallpapers at boot. With none in `Wallpapers/`, the shelf stays plain.

Per Shelf changes the wallpaper as you move between shelves. Name a file after each shelf:

| Shelf                  | File                            |
|------------------------|---------------------------------|
| Game Boy               | `GB.png`                        |
| Game Boy Color         | `GBC.png`                       |
| Game Boy Advance       | `GBA.png`                       |
| Game Boy Advance Video | `GBAV.png`, otherwise `GBA.png` |
| Favorites              | `Favorites.png`                 |
| Recents                | `Recents.png`                   |
| All Games (Kid Mode)   | `ALL.png`                       |

A shelf or collection without a file shows `Default.png`, or a plain background if there isn't one.

To give a [collection](collections.md) its own wallpaper, put a `.png` named after it in `Wallpapers/Collections/`,
like `Wallpapers/Collections/Puzzlers.png`. It shows behind that collection whatever your Wallpaper setting is.

| Device                                      | Suggested Size |
|---------------------------------------------|----------------|
| RG SP, RG 34XX, RG 34XXSP                   | 720×480        |
| RG 35XXSP, RG 35XX Plus, RG 35XX-H, RG 28XX | 640×480        |

Other sizes are scaled to fill the screen and cropped from the center.

If your wallpaper is too bright, the Wallpaper Tint setting can darken it a bit to make everything easier to read.

## Bezels

Drop a `.png` into `Bezels/GBA/`, `Bezels/GBC/` or `Bezels/GB/` and pick it in the Bezel row of the
`Settings → Screen` menu for the system.

The bezel is drawn over the game, so it needs a transparent hole where the game goes.

Where this hole is located in the bezel image depends on the Scale you have selected.

A bezel without a suffix will appear in the list regardless of the selected Scale. If the bezel filename
ends in one of the following suffixes, it will only appear when that Scale is selected.

| Scale     | Suffix      | Where the game goes                          |
|-----------|-------------|----------------------------------------------|
| SP Screen | `_spscreen` | In the middle, like a GBA SP shows Game Boy  |
| Integer   | `_integer`  | The biggest whole-number size, in the middle |
| Fit       | `_fit`      | As big as the whole picture fits             |

!!! info "Why would I use SP Screen over Integer?"

    SP Screen allows you to use a bottom bezel with Game Boy and Game Boy Color games.

    Paired with the bundled AGS-102 bezel, it makes your 4:3 device look like an OG GBA SP.

## Bezel templates

Each template is the screen in gray with a hole where the game goes. Paint over the gray and keep the hole clear.

Make your bezel the same size as your screen. slot scales any other size to fill the screen and crops what hangs over.

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

| Device                                                                              | Size                                 |
|-------------------------------------------------------------------------------------|--------------------------------------|
| RG 35XX Plus, RG 35XX 2024, RG 35XX-H, RG 35XX Pro, RG 35XXSP, RG 40XX-H, RG 40XX-V | 640×480                              |
| RG SP, RG 34XX, RG 34XXSP                                                           | 720×480                              |
| RG CubeXX                                                                           | 720×720                              |
| RG 28XX                                                                             | 480×640, turned 90° counterclockwise |

A smaller image is centred on black. A larger one, a 32-bit one or a compressed one won't show, and you'll get no logo at boot.

Delete `Config/bootlogo.bmp` to go back to the slot logo.
