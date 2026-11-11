# Save States

A save state is a snapshot of the whole game at one moment. They're separate from the game's own saves, which are called battery saves and are kept in `Saves/`.

## Making and loading states

| Button | Does |
| --- | --- |
| <kbd>SELECT</kbd> + <kbd>R1</kbd> | Save a state |
| <kbd>SELECT</kbd> + <kbd>L1</kbd> | Load the most recent state |
| Double tap <kbd>MENU</kbd> | Open the switcher |

Each game keeps up to 10 states. Saving an 11th throws out the oldest.

## The switcher

Double tap <kbd>MENU</kbd> in game, or pick States from the in-game menu. Your states are laid out like a stack of polaroids from newest to oldest.

| Button | Does |
| --- | --- |
| <kbd>Left</kbd> / <kbd>Right</kbd> | Flick through the states |
| <kbd>A</kbd> | Load the one on screen |
| <kbd>Y</kbd> | Delete the one on screen |
| <kbd>X</kbd> | Undo the last save or load |
| <kbd>B</kbd> | Back to the game |

Deleting a state requires confirmation. If you are sure press <kbd>X</kbd> to delete it. Note: once it's gone, it's gone.

## Undo

Saved over a good moment? Loaded the wrong one? Open the menu and select `Undo Load` or `Undo Save` within 30 seconds.

- Undoing a save removes the state you just made, and brings back the one it pushed out if you were already at 10.
- Undoing a load puts the game back the way it was before you loaded.

## The resume state

On top of your 10, slot keeps one more that you never see: the `resume state`. That's what a tap of <kbd>A</kbd> on the shelf loads.

slot writes it:

- every 5 minutes while you play
- when you close the lid or tap <kbd>POWER</kbd>
- when you power off
- when you eject
- when the battery hits 5% and slot powers off to protect your saves

There are two options in Settings → Gameplay that control this:

- **Resume on Insert** decides whether putting a cart in loads it.
- **Resume on Power On** decides whether powering on with a cart inserted loads the last state.

Turn both off and every game starts fresh.

## States and cores

States are tied to the emulator that made them. Switch a GBA game from mGBA to gpSP and you will have no save states. Switch back and they're all still there.

States live in `States/`. See [Storage Reference](storage.md).
