---
hide:
  - navigation
  - toc
---

# Updating slot.

<details class="way" id="over-the-air" open markdown>
<summary>Over the Air</summary>
Make sure you are connected to Wi-Fi.

From the shelf, open the menu and pick System → Check for Updates.

The OTA updater will reach out to GitHub to see if there is a new release of slot or BaseOS.

If updates are available, they show up as a cart.

Pick one and press <kbd>A</kbd> to download it. <kbd>B</kbd> cancels the download.

When updating BaseOS, slot will check your battery level. If you are below 20% and not currently charging, you will be prompted to plug in. Press <kbd>A</kbd> to update anyway.

</details>

<details class="way" id="manual" markdown>
<summary>Manual</summary>

Update from your computer with the card in hand.

!!! warning "Updating from a version before 1.4.0?"

    Older versions kept settings in `System/`. They now live in `Config/`.

    For this update only, copy the new `System/` over the old one instead of deleting it. slot will move your settings into a new `Config/` folder, so future updates can delete `System/` as normal.

    Your games, saves and save states aren't affected.

    Sorry for the hassle! slot started as a throwaway project and I never expected it to need an update procedure.

<div class="install-steps" markdown>

1.  Download the latest release
    { .step-title role="heading" aria-level="3" }

    Go fetch the [latest slot release](https://github.com/slot-cfw/slot/releases/latest) from GitHub and unzip it.

2.  Replace the `System/` folder
    { .step-title role="heading" aria-level="3" }

    Connect the card that holds slot to your computer. That is the card in `SD1` if you use a single card, or the card in `SD2` if you use two.

    Delete the `System/` folder on the card and copy the new `System/` folder from the release in its place.
    Your games, saves, save states, labels and wallpapers all live outside `System/` and are left alone.

3.  Put the card back and boot!
    { .step-title role="heading" aria-level="3" }

    I hope you continue to enjoy using slot!

</div>

</details>
