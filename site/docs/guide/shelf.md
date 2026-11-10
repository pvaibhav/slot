# The Shelf

## Getting around

| Button | Does |
| --- | --- |
| <kbd>Left</kbd> / <kbd>Right</kbd> | Move along the shelf |
| <kbd>Up</kbd> / <kbd>Down</kbd> | Jump to the previous or next letter |
| <kbd>L1</kbd> / <kbd>R1</kbd> | Switch shelves |
| <kbd>Y</kbd> | Favorite / unfavorite the cart you're on |
| <kbd>X</kbd> | Take the cart you're on off Recents |
| <kbd>A</kbd> | Insert the cart |
| <kbd>Hold A</kbd> | Insert the cart and don't auto load the save state |
| <kbd>START</kbd> | Choose the emulator (GBA only) |
| <kbd>MENU</kbd> | Open settings |

## The shelves

There's one shelf per system, along with two special shelves, in this order:

1. Recents, if enabled in settings
2. Favorites, once you've hearted something
3. Game Boy Advance
4. Game Boy Advance Video
5. Game Boy Color
6. Game Boy

slot does not show empty shelves. If you only have GBA games, you only get the GBA shelf.

Game Boy Advance Video carts can be optionally placed in `Games/GBAV`. Here they get their own shelf and Game Boy Player support is disabled when using mGBA.

Within a shelf, games are sorted by name. Names starting with a number come first.

slot remembers which shelf and cart you were on when it gets powered off and will return there on the next boot. If you powered down while still in game it will resume that game on the next boot.

## Favorites

Press <kbd>Y</kbd> to heart a cart. It shows up on the Favorites shelf alongside any other favorites across all three platforms. Press <kbd>Y</kbd> again to take the heart off.

## Recents

Turn on Recents in Settings → Shelf to get a shelf of the last 10 games you played.

Press <kbd>X</kbd> on the Recents shelf to remove the selected game.

## Game titles

Turn on Game Titles in Settings → Shelf and they will appear above the slot.

## Adding games

New games, labels and cart shells are picked up when slot boots.

See [Storage Reference](storage.md) for where games go and [Customization](customizing.md) for labels / shells.
