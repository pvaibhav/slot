# Playing Games

## Inserting a cart

Tap <kbd>A</kbd> on a cart to insert it. If you've played it before, you pick up right where you left off.

Hold <kbd>A</kbd> to start the game fresh. The game will launch without autoloading your save state.

If you'd rather always start fresh, set Auto Load State to Never in Settings → Gameplay. Power On starts carts fresh but still resumes the game you powered off in.

To restart a game while you're playing it, bind Reset Game in [Shortcuts](controls.md). Pressed it by mistake? Open the menu and select `Undo Reset` within 30 seconds.

## Ejecting

Hold <kbd>MENU</kbd> to eject the cart and go back to the shelf. 

slot creates a save state on the way out, unless you've turned off Eject Save State in Settings → Gameplay.

## The in-game menu

Tap <kbd>MENU</kbd> while playing. The game pauses behind it.

| Row | Does |
| --- | --- |
| Resume | Back to the game |
| States | Opens the [save state switcher](save-states.md) |
| Screenshots | Opens the [screenshot gallery](screenshots.md) |
| Settings | Screen and Gameplay for the system you're playing, and Controls |
| Exit Game | Ejects the cart |

## Picking an emulator

GBA games can use [mGBA](https://mgba.io) or [gpSP](https://github.com/libretro/gpsp). Game Boy and Game Boy Color games can use [Gambatte](https://github.com/libretro/gambatte-libretro) or mGBA. Gambatte is the default on new cards.

On the shelf, press <kbd>START</kbd> on a GBA, GBC or GB cart. It opens to show the circuit board with a chip in one of two sockets. <kbd>Left</kbd> puts it on mGBA, <kbd>Right</kbd> on gpSP or gambatte (GB/C Only). <kbd>A</kbd> saves your choice, <kbd>B</kbd> backs out.

slot remembers the choice per game.

Which should you use?

- mGBA is more accurate but at the expense of using more resources.
- gpSP is faster but less accurate. It's also the one that does the wireless adapter for Pokémon trading. See [Linking Up](linking.md).
- gambatte is extremely popular and accurate for GB/C. Link play is currently not supported in gambatte.

!!! warning "Save states belong to a core"

    If you switch cores, you won't be able to use your existing save states. They will be avaliable when you switch back. 
    
    Your battery save is shared, so in-game saves carry over.

## Battery saves

Games that save on the cart write to `Saves/`.

## Rumble

Games with a rumble pak rumble. Turn it off in Settings → Gameplay if you'd rather they didn't.

Pokémon Pinball: Ruby & Sapphire still needs rumble turned on in the game's own options.

## Solar sensor

On OG hardware, the Boktai games have a light sensor built into the cart. 

mGBA emulates this hardware. By default, slot sets the value based on the clock. The sun comes up at 6 AM, is strongest at noon and is gone by 6 PM.

To manually pick the sun level yourself, bind Sunlight Up and Sunlight Down in [Shortcuts](controls.md). Your manual adjustment will only persist for that session.

!!! important "gpSP does not have solar sensor support"

    Be sure to run the Boktai games with the mGBA core.

## BIOS

slot doesn't need any BIOS files. If you have your own, drop them into `BIOS/` and you'll get the real boot animation
when a game starts.

- `gba_bios.bin`
- `gbc_bios.bin`
- `gb_bios.bin`

If you link with mGBA, both devices need the same BIOS setup. See [Linking Up](linking.md).
