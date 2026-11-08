# Gamebit

Gamebit is a web page for managing your card over Wi-Fi. Use it from a phone or computer on the same network to add games, saves, labels and more without taking the card out.

## Starting Gamebit

You need Wi-Fi set up first. Open the menu on the shelf and go to Network to join a network.

Once you are connected to a Wi-Fi network:

1. Open the menu on the shelf and pick Network → Gamebit.
2. The device prints a receipt with a QR code and a PIN.
3. Scan the QR code on the receipt, or go to slot.local and enter the PIN.

The PIN changes every time you open Gamebit. If you close it and open it again, the page will ask for the new one.

Some routers do not honor / support mDNS. If `slot.local` doesn't load, use the device IP shown instead.

Press <kbd>B</kbd> on the device to stop Gamebit and go back to the menu.

## Games

The Games tab lists your platforms. Open one to see its games in a card or list view.

Use Upload ROMs or drop files on the page to add games. Select lets you delete several at once.

Open a game to rename it, delete it, or manage its files. Deleting a game removes the ROM. Tick the box to remove its saves, save states, label and guide too.

## Saves

The Saves tab on a game lists its save files. You can upload, download and delete them. slot supports `.sav` or `.srm` files.

!!! warning "Uploaded saves and auto-loaded states"

    If the game has a save state it will be auto-loaded and not read the newly uploaded save. Hold <kbd>A</kbd> the next time you start the game to use the save.

The Save States tab shows your save states with their previews, and you can delete them.

## Labels

Open a game to change its label.

- **Custom Label** uploads your own `.png`, up to 8 MB.
- **Fetch Label** gets one from the art server. It asks first if the game already has a label.
- **Remove label** deletes it.

See [Cart labels](customizing.md#cart-labels) for suggested sizes.

## Guides

The Guides tab on a game takes `.txt` files up to 4 MB. See [Game Guides](game-guides.md).

## Bezels

Each platform has a Bezels tab for uploading and removing bezels. See [Bezels](customizing.md#bezels).

## BIOS

Each platform has a BIOS tab. BIOS files are optional. See [BIOS](playing.md#bios).

## Boot logo

The Customization tab has a Boot Logo page. Upload a `.bmp`, or pick Use slot logo to go back.

A new logo shows on the second power-on. See [Boot logo](customizing.md#boot-logo).

## Wallpapers

The Customization tab also has your wallpapers. See [Wallpapers](customizing.md#wallpapers).

## Files

The File Manager under Tools browses the card. You can upload, download, rename, move and delete files.

Top-level folders like `Games` and `Saves` can't be deleted, renamed or moved.
