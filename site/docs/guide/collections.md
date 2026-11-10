# Collections

A collection is a shelf of games you pick yourself. It can mix Game Boy, Game Boy Color and Game Boy Advance games, and you can have as many as you like.

## Browsing

Collections stack up behind Favorites. On the Favorites shelf, press <kbd>R2</kbd> for the next collection and <kbd>L2</kbd> for the previous one. The stack wraps around.

slot remembers which collection you were looking at. Switch away with <kbd>L1</kbd> / <kbd>R1</kbd> and come back, or power off, and you land on the same one.

Empty collections are skipped. If you have no favorites, your first collection takes the Favorites spot.

## Shelf Mode

Settings → Shelf → Shelf Mode has two choices:

- **All**: the system shelves as usual, with your collections stacked behind Favorites.
- **Collections**: the system shelves are hidden and each collection gets its own spot in the <kbd>L1</kbd> / <kbd>R1</kbd> row, after Recents and Favorites. <kbd>L2</kbd> / <kbd>R2</kbd> do nothing.

## Adding games

Hold <kbd>Y</kbd> on a game. It starts to jiggle, and a moment later your collections appear as stacks of their own carts. Press <kbd>Left</kbd> / <kbd>Right</kbd> to pick one and <kbd>A</kbd> to add the game. <kbd>B</kbd> goes back.

A quick tap of <kbd>Y</kbd> still favorites the game.

Collections that already have the game aren't shown. The last one you added to is picked first. The stack at the end, New Collection, opens the keyboard to name a new one. Press <kbd>START</kbd> when you're done, or <kbd>Y</kbd> to go back.

### Several at once

1. Hold <kbd>SELECT</kbd> on the shelf. Games you check jiggle gently.
2. Move around as usual and press <kbd>A</kbd> to check or uncheck the cart in front of you. You can check games on any shelf.
3. Press <kbd>START</kbd>, then pick a collection and press <kbd>A</kbd>.

Games already in that collection are skipped. <kbd>B</kbd> on the collections goes back to your checked games.

## Removing a game

On a collection's shelf, hold <kbd>Y</kbd> on the game, then press <kbd>A</kbd> to remove it or <kbd>B</kbd> to keep it. Removing the last game hides the collection until you add something to it again.

## Moving things around

On a collection's shelf:

- Press <kbd>X</kbd> to pick up the game you're on. Use <kbd>Left</kbd> / <kbd>Right</kbd> to move it, holding to keep going, then <kbd>A</kbd> to drop it or <kbd>B</kbd> to put it back.
- Keep holding <kbd>X</kbd> to pick up the whole collection instead and move it among your other collections the same way.

Renaming or deleting a collection is done on the card.

## On the card

```
Collections/
  order.txt        the stack order, one name per line
  Puzzlers.txt     one game per line
Wallpapers/
  Collections/
    Puzzlers.png
```

Each line in a collection file is the system and the file name without its extension, like `gba/Apotris` or `gb/Catrap (USA)`. The shelf shows games in the order they're listed.

A collection with a picture in `Wallpapers/Collections/` named after it shows that picture behind its shelf, whatever your Wallpaper setting is.
