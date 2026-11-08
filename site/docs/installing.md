---
hide:
  - navigation
  - toc
---

# Installing slot.

<details class="way" id="one-sd-card" open markdown>
<summary>One SD Card</summary>

<div class="install-steps" markdown>

1.  Flash BaseOS to an SD card
    { .step-title role="heading" aria-level="3" }

    Download the latest release of
    [BaseOS](https://github.com/pvaibhav/BaseOS/releases/latest) and write the `.img` to an SD card with
    something like [Rufus](https://rufus.ie) or [Raspberry Pi Imager](https://www.raspberrypi.com/software/).

    Insert this card into the SD card slot labeled `SD1`.

2.  Expand the storage
    { .step-title role="heading" aria-level="3" }

    After inserting the SD card, power on your device. You should see a BaseOS boot logo and a message saying that storage is being expanded.

    Once the storage is expanded, a message will say that no frontend was found. Power off your device.

3.  Download the latest release
    { .step-title role="heading" aria-level="3" }

    Go fetch the [latest slot release](https://github.com/slot-cfw/slot/releases/latest) from GitHub.

4.  Copy it onto your SD card
    { .step-title role="heading" aria-level="3" }

    Eject the SD card from `SD1` and connect it back to your computer. Unzip the release zip you downloaded and copy the contents to the root of the SD card.

5.  Add your games
    { .step-title role="heading" aria-level="3" }

    Drop your legally obtained ROM files into their corresponding platform's folder in `Games/`. You may optionally place labels into the matching platform folder found in
    `Labels/`.

    !!! warning "ROMs must be unzipped!"

        slot does not read `.zip` or `.7z` files.

6.  Re-insert the card into `SD1` and boot!
    { .step-title role="heading" aria-level="3" }

    I hope you enjoy using slot!

</div>

</details>

<details class="way" id="two-sd-cards" markdown>
<summary>Two SD Cards</summary>

<div class="install-steps" markdown>

1.  Flash BaseOS to an SD card
    { .step-title role="heading" aria-level="3" }

    Download the latest release of
    [BaseOS](https://github.com/pvaibhav/BaseOS/releases/latest) and write the `.img` to an SD card with
    something like [Rufus](https://rufus.ie) or [Raspberry Pi Imager](https://www.raspberrypi.com/software/).

    Insert this card into the SD card slot labeled `SD1`.

2.  Download the latest release
    { .step-title role="heading" aria-level="3" }

    Go fetch the [latest slot release](https://github.com/slot-cfw/slot/releases/latest) from GitHub.

3.  Copy it onto SD2
    { .step-title role="heading" aria-level="3" }

    Prepare your second SD card by formatting it to `FAT32` or `exFAT` with a `Master Boot Record (MBR)` partition scheme.

    Once formatted, unzip the release zip you downloaded and copy the contents to the second SD card.

4.  Add your games
    { .step-title role="heading" aria-level="3" }

    Drop your legally obtained ROM files into their corresponding platform's folder in `Games/`. You may optionally place labels into the matching platform folder found in
    `Labels/`.

    !!! warning "ROMs must be unzipped!"

        slot does not read `.zip` or `.7z` files.

5.  Put the second card into `SD2` and boot!
    { .step-title role="heading" aria-level="3" }

    I hope you enjoy using slot!

</div>

</details>
