# The Screen

Each system gets its own screen settings, so you can have a grid on your GBA games and Simpletex on your Game Boy ones.
Find them in Settings → Screen, or in the in-game menu while you play.

## Shaders

Shaders make the picture look more like a real handheld screen.

| GBA            | Game Boy and Game Boy Color |
|----------------|-----------------------------|
| Off            | Off                         |
| LCD3x          | LCD3x                       |
| Grid (default) | Grid                        |
| Dot            | Simpletex (default)         |
| Shimmerless    | Shimmerless                 |

Shimmerless is only available for GBA on 4:3 devices. GBA on a 3:2 device is always exactly 3x resolution and Shimmerless has no visual impact.

<kbd>SELECT</kbd> + <kbd>A</kbd> and <kbd>SELECT</kbd> + <kbd>B</kbd> step through them while you play.

## Color correction

The original screens were dim and washed out, and developers took this into account while making their games. Color
correction pulls the colors back toward what you'd have seen on original hardware. It's off by default.

<kbd>SELECT</kbd> + <kbd>Y</kbd> toggles it in game.

## Game Boy palettes

Original Game Boy games can be shown in other colors. Pick one under Settings → Screen → Game Boy → Palette:

- **Auto** lets mGBA pick, the way a Game Boy Color colors in old games.
- **Grayscale**, **DMG Green**, **GB Pocket** and **GB Light** for the classics.
- The 12 Game Boy Color presets and 32 Super Game Boy palettes.

<kbd>SELECT</kbd> + <kbd>L2</kbd> / <kbd>R2</kbd> step through the same list while you play.

Palettes only apply to original Game Boy games.

## Scale

Scale decides how big the picture is. Pick it under Settings → Screen for each system.

The choices depend on your screen's shape. slot leaves out any that would look the same on your device.

### 4:3 Devices (640×480)

RG35XXSP, RG35XX Plus, RG35XX-H and RG28XX

**Game Boy and Game Boy Color**

| Scale     | Does                                                                       |
|-----------|----------------------------------------------------------------------------|
| SP Screen | The way a GBA SP shows them: in the middle of a GBA-sized screen (default) |
| Integer   | The biggest whole-number size                                              |
| Fit       | As big as the whole picture fits                                           |

**GBA**

| Scale       | Does                                                 |
|-------------|------------------------------------------------------|
| Fit         | As big as the picture fits above the bezel (default) |
| Integer     | The biggest whole-number size                        |
| Full Screen | Fills the whole screen, over the bezel               |

### 3:2 Devices (720×480)

RG SP, RG34XX and RG34XXSP

| Scale   | Game Boy and Game Boy Color                                                             |
|---------|-----------------------------------------------------------------------------------------|
| Integer | The biggest whole-number size. <br/>This is also how a real GBA SP shows them (default) |
| Fit     | As big as the whole picture fits                                                        |

GBA has no Scale row on these devices.

## Stretching

Just like the OG, <kbd>L1</kbd> stretches Game Boy and Game Boy Color games across the screen. <kbd>R1</kbd> puts them
back. slot remembers your pick per game.

The Stretch setting decides how far <kbd>L1</kbd> stretches them:

- **Classic** stretches across a GBA-sized screen, like a GBA SP. This is the default.
- **Full Screen** stretches to the edges of the whole screen.

Bezels are hidden while a game is stretched.

## Frame blending

Some games flicker sprites on and off every other frame to fake see-through effects. The original screens were slow
enough to blur that into a steady image. On a modern screen you see the flicker.

| Frame Blending | Does                                                        |
|----------------|-------------------------------------------------------------|
| Off            | Shows every frame as it is (default)                        |
| Mix            | Blends each frame with the one before                       |
| Ghosting       | Leaves a fading trail, like the original screens. mGBA only |

A game on gpSP set to Ghosting gets Mix.

## Bezels

A bezel is a frame drawn around the game.

- **Game Boy and Game Boy Color**: None, Default, AGS-102 on 640×480 devices, or your own from `Bezels/`. Default has
  its own versions for the Integer and Fit scales.
- **Game Boy Advance**: only on 640×480 devices with the Fit scale, where AGS-102 draws the bottom of a GBA SP under the
  game.

Bezels are hidden when the picture fills the screen.

See [Customization](customizing.md) for details on how to make your own.

## Brightness and blue light

| Button                                                 | Does       |
|--------------------------------------------------------|------------|
| <kbd>SELECT</kbd> + <kbd>Up</kbd> / <kbd>Down</kbd>    | Brightness |
| <kbd>SELECT</kbd> + <kbd>Left</kbd> / <kbd>Right</kbd> | Blue light |

These work on the shelf and in game. They're also rows in Settings → Screen → General.

Blue light warms the picture up for playing in the dark. Every brightness step keeps the backlight on, so the lowest one
is still readable in the dark.

## Saturation and contrast

You can adjust saturation and contrast in two places. Settings → Screen → General if on the shelf and at the end of the in-game
Screen page. Both go from Off to 9 and are applied globally.