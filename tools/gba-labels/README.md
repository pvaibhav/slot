# GBA cartridge label downloader

Standalone Python 3.9+ script. Requires Pillow and curl. Nothing is installed automatically. Artwork is downloaded from the public LaunchBox Games Database; ROM contents are never read or uploaded.

## Install dependencies

Run these commands from the repository root. Install `curl` separately if it is not already available.

```sh
python3 -m venv ~/.venvs/slot-labels
~/.venvs/slot-labels/bin/pip install -r tools/gba-labels/requirements.txt
```

## Preview missing labels

```sh
~/.venvs/slot-labels/bin/python tools/gba-labels/gba_labels.py /Volumes/Slot/Games
```

The default destination is the sibling `Labels` folder. Existing valid 196×86 PNGs are skipped before any network request. New labels, source images, a contact sheet, and `report.json` are placed in a temporary folder whose path is printed. Preview mode does not modify the destination.

## Download and copy missing labels

```sh
~/.venvs/slot-labels/bin/python tools/gba-labels/gba_labels.py /Volumes/Slot/Games --apply
```

`--apply` authorizes writing the newly prepared labels. Existing files are never overwritten. Invalid existing labels are reported and left untouched. Each written file is read back and compared with the prepared PNG.

Options:

- `--labels-dir /path/to/Labels`: choose a destination.
- `--recursive`: scan subfolders and mirror their structure inside Labels.
- `--cache-dir /tmp/gba-art-cache`: reuse downloaded pages and images between runs.
- `--allow-region-fallback`: explicitly permit another region's cartridge art.
- `--overrides /path/to/overrides.json`: resolve a game or adjust a crop manually.

Only `.gba` files are supported (case insensitive); macOS `._` metadata files are ignored. Filenames retain all original punctuation and tags, with only the extension changed to `.png`.

## Matching and crops

Eleven common games have verified database IDs built in. Other games are searched by title with ROM region/revision tags removed, restricted to GBA, and accepted only on a unique normalized exact match. Ambiguous or missing matches are reported rather than guessed. Regional variants use the ROM's region when recognized, otherwise North America.

The script uses three supported cartridge image layouts. It crops the label, preserves its aspect ratio, and resizes to 196×86 with Lanczos resampling. The top of the label is retained; some fine print at the bottom is cropped. These are layout heuristics, not automatic label detection: inspect the preview for newly encountered artwork. Other source dimensions require an explicit crop. Original downloaded images are saved under the printed temporary folder's `sources` directory for inspection.

An override file is a JSON object keyed by the exact relative ROM filename:

```json
{
  "Example Game (USA).gba": {
    "game_id": 12345,
    "region": "North America",
    "crop": [137, 130, 865, 497],
    "anchor": 0.0
  }
}
```

Replace the example ID and crop with real values. Crop coordinates are pixels in the source image: left, top, right, bottom. `anchor` controls the final vertical crop: 0 retains the top, 0.5 centers, 1 retains the bottom. All override fields are optional. `image_url` can specify an exact HTTPS `images.launchbox-app.com` image instead of resolving a game.

A run processes all games even if some fail, writes a report, and exits with status 1 if any failed. Re-running skips successful copies. Downloads have size limits, timeouts, and retries. The public site's format can change; parser failures appear as missing matches/artwork in the report.

Previews, source images, and reports are written to the system temporary directory. Copy any artifacts you want to keep before system cleanup. ROMs and downloaded artwork are not included in this repository.
