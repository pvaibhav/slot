# Storage Reference

slot keeps everything on the card in plain folders.

If you use two cards, all of this is on the one in `SD2`.

## The folders

| Folder                                  | What's in it                        |
|-----------------------------------------|-------------------------------------|
| `Games/GBA`, `Games/GBAV`, `Games/GB`, `Games/GBC`     | Your games                          |
| `Labels/GBA`, `Labels/GBAV`, `Labels/GB`, `Labels/GBC` | Cart labels                         |
| `Saves/`                                | Battery saves, the game's own saves |
| `States/`                               | Save states                         |
| `Screenshots/`                          | Your screenshots                    |
| `BIOS/`                                 | Optional BIOS files                 |
| `Wallpapers/`                           | Your wallpapers                     |
| `Bezels/GBA`, `Bezels/GB`, `Bezels/GBC` | Your bezels                         |
| `Overlays/GBA`, `Overlays/GB`, `Overlays/GBC` | Your overlays                 |
| `Config/`                               | Your settings                       |
| `Logs/`                                 | Logs, for when something breaks     |
| `System/`                               | slot itself                         |

slot makes any of these that are missing when it boots, except `Screenshots/`, which appears with your first screenshot.

## Games

| Folder      | Takes         |
|-------------|---------------|
| `Games/GBA` | `.gba`        |
| `Games/GBAV` | `.gba`       |
| `Games/GB`  | `.gb`, `.gbc` |
| `Games/GBC` | `.gb`, `.gbc` |

The folder decides which shelf a game goes on.

!!! warning "ROMs must be unzipped!"

    slot does not read `.zip` or `.7z` files.

Put games right in the folder. slot doesn't look inside subdirectories.

## Names matter

Everything about a game hangs off its file name: its battery save, save states, label, cart shell and emulator choice.

If you rename `Metroid Fusion.gba`, rename its save in `Saves/GBA` too, or slot won't find it.

## Saves and states

- Battery saves are `Saves/<system>/<game>.sav`.
- Save states are in `States/<system>/<emulator>/<game>/`. Each one has a `.png` next to it for the switcher.
- Screenshots are `Screenshots/<system>/<game>/<date and time>.png`.

Periodically back up `Saves/` and `States/`. You never know when an SD card will go bad.

## Config

Most of what's in `Config/` is written by slot when you change a setting. 

I don't recommend editing these by hand but it's your life.

| File                                            | What it holds                                      |
|-------------------------------------------------|----------------------------------------------------|
| `slot.state`                                    | Your settings and where you left off               |
| `controls.txt`, `shortcuts.txt`, `carousel.txt` | Your button layout                                 |
| `favorites.txt`, `recents.txt`                  | Your Favorites and Recents                         |
| `selected_core.txt`                             | Which emulator each GBA game uses                  |
| `video_mode.txt`                                | Which Game Boy games you've stretched with L1      |
| `cart_shell.txt`                                | Cart colors, see [Customization](customizing.md). A `Labels/cart_shell.txt` overrides it |
| `bootlogo.bmp`                                  | Your boot logo, if you've made one                 |

If you ever want slot back to how it came, delete `Config/`. 

You'll have to set the clock again on the next boot.

## Updating

To update, download the latest release and replace the `System/` folder.

See [Updating](../updating.md) for detailed instructions.
