#!/usr/bin/env python3
"""Prepare missing 196x86 GBA cartridge labels. Requires Python 3.9+, Pillow, curl."""
import argparse
import hashlib
import html
from html.parser import HTMLParser
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
import unicodedata
from urllib.parse import quote, urlparse

try:
    from PIL import Image, ImageDraw, ImageOps
except ImportError:
    sys.exit('Install Pillow first: python3 -m pip install Pillow')

BASE = 'https://gamesdb.launchbox-app.com'
SIZE = (196, 86)
# Verified database entries for common titles.
KNOWN = {
    'advance wars': 2367, 'aladdin': 3215,
    'dexters laboratory deesaster strikes': 26190, 'lady sia': 14182,
    'legend of zelda a link to the past and four swords': 8376,
    'metal slug advance': 10743, 'metroid zero mission': 3552,
    'super mario advance': 2225, 'tetris worlds': 3827,
    'tom clancys rainbow six rogue spear': 3412,
    'warioware inc mega microgames': 3917,
}

def title_for(stem):
    value = re.sub(r'\s*(\([^)]*\)|\[[^]]*\])', '', stem).strip()
    value = re.sub(r',\s*The\b', '', value, flags=re.I)
    return value


def normalized(value):
    value = unicodedata.normalize('NFKD', html.unescape(value)).encode('ascii', 'ignore').decode()
    value = value.lower().replace('&', ' and ').replace('$', 's')
    value = re.sub(r"[’']", '', value)
    value = re.sub(r'[^a-z0-9]+', ' ', value).strip()
    return re.sub(r'^(the |disneys )', '', value)


class SearchParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.results = {}
        self.current = None
        self.heading = False

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == 'a' and re.match(r'/games/details/\d+', attrs.get('href', '')):
            self.current = {'path': attrs['href'], 'title': '', 'text': []}
        if self.current and tag == 'h3':
            self.heading = True

    def handle_data(self, data):
        if self.current:
            self.current['text'].append(data.strip())
            if self.heading:
                self.current['title'] += data

    def handle_endtag(self, tag):
        if tag == 'h3':
            self.heading = False
        if tag == 'a' and self.current:
            item = self.current
            if 'Nintendo Game Boy Advance' in item['text'] and item['title']:
                self.results[item['path']] = item
            self.current = None


class ArtParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.images = []

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if tag == 'a' and ' - Cart - Front Image' in a.get('data-title', ''):
            if 'Fanart' not in a['data-title'] and a.get('href', '').startswith('https://images.launchbox-app.com/'):
                self.images.append({'url': a['href'], 'title': a['data-title']})


class Downloads:
    def __init__(self, cache):
        self.cache = cache
        cache.mkdir(parents=True, exist_ok=True)

    def get(self, url):
        parsed = urlparse(url)
        if parsed.scheme != 'https' or parsed.hostname not in {'gamesdb.launchbox-app.com', 'images.launchbox-app.com'}:
            raise ValueError('Only HTTPS LaunchBox URLs are accepted: ' + url)
        dest = self.cache / hashlib.sha256(url.encode()).hexdigest()
        if dest.exists():
            return dest.read_bytes()
        last = ''
        for attempt in range(3):
            # A query variation also avoids occasionally stalled image CDN responses.
            candidate = url if not attempt else url + ('&' if '?' in url else '?') + f'label_retry={attempt}'
            result = subprocess.run(['curl', '--fail', '--location', '--silent', '--show-error',
                '--proto', '=https', '--proto-redir', '=https', '--connect-timeout', '15',
                '--max-time', '60', '--max-filesize', '25000000', candidate], capture_output=True)
            if result.returncode == 0 and result.stdout:
                data = result.stdout
                try:
                    if parsed.hostname == 'images.launchbox-app.com':
                        with Image.open(io.BytesIO(data)) as im:
                            im.verify()
                    elif b'<html' not in data.lower():
                        raise ValueError('Response was not an HTML page')
                except Exception as exc:
                    last = str(exc)
                else:
                    dest.write_bytes(data)
                    time.sleep(0.3)
                    return data
            else:
                last = result.stderr.decode(errors='replace').strip()
            time.sleep(attempt + 1)
        raise RuntimeError('Download failed: ' + url + ' — ' + last)


def resolve_game(title, downloads):
    key = normalized(title)
    if key in KNOWN:
        return f'/games/images/{KNOWN[key]}'
    parser = SearchParser()
    parser.feed(downloads.get(BASE + '/games/results?id=' + quote(key)).decode())
    exact = [x for x in parser.results.values() if normalized(x['title']) == key]
    if len(exact) != 1:
        candidates = ', '.join(x['title'] + ' (' + x['path'] + ')' for x in parser.results.values())
        raise ValueError('No unique exact GBA title match. Use an override with game_id. Candidates: ' + (candidates or 'none'))
    return exact[0]['path'].replace('/details/', '/images/')


def preferred_regions(filename):
    tags = ' '.join(re.findall(r'\(([^)]*)\)', filename)).lower()
    if 'usa' in tags or 'world' in tags:
        return ['North America', 'United States']
    for token, regions in [('europe', ['Europe']), ('japan', ['Japan']),
                           ('australia', ['Australia', 'Oceania'])]:
        if token in tags:
            return regions
    return ['North America', 'United States']


def choose_art(page, rom, downloads, override, allow_fallback):
    if 'image_url' in override:
        return {'url': override['image_url'], 'title': 'Manual artwork override'}
    parser = ArtParser()
    parser.feed(downloads.get(page).decode())
    regions = [override['region']] if 'region' in override else preferred_regions(rom.name)
    for region in regions:
        for image in parser.images:
            if image['title'].endswith('(' + region + ')'):
                return image
    if allow_fallback and parser.images:
        return parser.images[0]
    raise ValueError('No cartridge front for region ' + '/'.join(regions) + '. Use --allow-region-fallback or an override.')


def prepare_image(data, override):
    with Image.open(io.BytesIO(data)) as source:
        im = ImageOps.exif_transpose(source).convert('RGB')
    w, h = im.size
    if 'crop' in override:
        box = override['crop']
    elif (w, h) == (1000, 574):
        box = [137, 130, 865, 497]
    elif (w, h) == (600, 355):
        box = [82, 82, 520, 305]
    elif (w, h) == (473, 283):
        box = [69, 68, 402, 246]
    else:
        raise ValueError(f'Unreviewed cartridge layout {w}x{h}; set crop: [left, top, right, bottom] in an override.')
    if len(box) != 4 or not (0 <= box[0] < box[2] <= w and 0 <= box[1] < box[3] <= h):
        raise ValueError('Crop must be four pixel coordinates within the source image')
    anchor = override.get('anchor', 0.0)
    if not isinstance(anchor, (int, float)) or not 0 <= anchor <= 1:
        raise ValueError('anchor must be between 0 (top) and 1 (bottom)')
    return ImageOps.fit(im.crop(tuple(box)), SIZE, method=Image.Resampling.LANCZOS,
                        centering=(0.5, anchor)), box


def valid_label(path):
    try:
        with Image.open(path) as im:
            valid = im.format == 'PNG' and im.size == SIZE
            im.verify()
            return valid
    except (OSError, ValueError):
        return False


def previews(items, stage):
    paths = []
    for start in range(0, len(items), 60):
        group = items[start:start + 60]
        sheet = Image.new('RGB', (900, ((len(group) + 2) // 3) * 145), '#f4f2ee')
        draw = ImageDraw.Draw(sheet)
        for i, item in enumerate(group):
            x, y = (i % 3) * 300 + 14, (i // 3) * 145 + 8
            # Filename mapping is fully preserved in report.json.
            label = title_for(Path(item['rom']).stem)
            draw.text((x, y), label[:43], fill='#242424')
            with Image.open(stage / 'Labels' / item['output']) as im:
                sheet.paste(im, (x, y + 25))
        dest = stage / f'preview-{start // 60 + 1}.png'
        sheet.save(dest)
        paths.append(str(dest))
    return paths


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('rom_folder', type=Path)
    p.add_argument('--labels-dir', type=Path, help='Default: Labels beside the ROM folder')
    p.add_argument('--apply', action='store_true', help='Also copy prepared labels to the destination; never overwrite')
    p.add_argument('--recursive', action='store_true', help='Mirror ROM subfolders in Labels')
    p.add_argument('--overrides', type=Path, help='JSON mapping relative ROM filenames to game_id, image_url, region, crop, anchor')
    p.add_argument('--cache-dir', type=Path, help='Optional persistent download cache')
    p.add_argument('--allow-region-fallback', action='store_true', help='Use another region when requested artwork is missing')
    args = p.parse_args()
    rom_dir = args.rom_folder.expanduser().resolve()
    if not rom_dir.is_dir():
        p.error('ROM folder does not exist: ' + str(rom_dir))
    if not shutil.which('curl'):
        p.error('curl must be installed')
    dest = (args.labels_dir.expanduser() if args.labels_dir else rom_dir.parent / 'Labels').resolve()
    overrides = json.loads(args.overrides.read_text()) if args.overrides else {}
    if not isinstance(overrides, dict):
        p.error('Overrides must be a JSON object')
    stage = Path(tempfile.mkdtemp(prefix='gba-labels-'))
    (stage / 'Labels').mkdir()
    downloads = Downloads(args.cache_dir.expanduser() if args.cache_dir else stage / 'cache')
    roms = sorted(x for x in (rom_dir.rglob('*') if args.recursive else rom_dir.iterdir())
                  if x.is_file() and x.suffix.lower() == '.gba' and not x.name.startswith('._'))
    report = {'rom_folder': str(rom_dir), 'destination': str(dest), 'applied': args.apply,
              'prepared': [], 'skipped': [], 'errors': []}
    outputs = set()
    for rom in roms:
        rel = rom.relative_to(rom_dir)
        out = rel.with_suffix('.png')
        try:
            if out.as_posix().casefold() in outputs:
                raise ValueError('Multiple ROM filenames would produce the same label')
            outputs.add(out.as_posix().casefold())
            target = dest / out
            if not target.resolve().is_relative_to(dest):
                raise ValueError('Destination symlink leads outside Labels folder')
            if target.exists():
                if not valid_label(target):
                    raise ValueError('Existing label is invalid or not 196x86; left untouched: ' + str(target))
                report['skipped'].append(str(rel))
                print('SKIP ', rel, flush=True)
                continue
            override = overrides.get(rel.as_posix(), {})
            title = title_for(rom.stem)
            page = BASE + (f'/games/images/{int(override["game_id"])}' if 'game_id' in override
                           else resolve_game(title, downloads)) if 'image_url' not in override else None
            art = choose_art(page, rom, downloads, override, args.allow_region_fallback)
            data = downloads.get(art['url'])
            source = stage / 'sources' / out
            source.parent.mkdir(parents=True, exist_ok=True)
            with Image.open(io.BytesIO(data)) as im:
                im.save(source, format='PNG')
            im, crop = prepare_image(data, override)
            prepared = stage / 'Labels' / out
            prepared.parent.mkdir(parents=True, exist_ok=True)
            im.save(prepared, format='PNG')
            assert valid_label(prepared)
            item = {'rom': str(rel), 'output': str(out), 'page': page, **art, 'crop': crop}
            if args.apply:
                target.parent.mkdir(parents=True, exist_ok=True)
                payload = prepared.read_bytes()
                # Exclusive creation prevents overwriting an existing label, including on a rerun.
                with target.open('xb') as handle:
                    handle.write(payload)
                    handle.flush()
                    os.fsync(handle.fileno())
                if target.read_bytes() != payload:
                    raise OSError('Destination verification failed: ' + str(target))
            report['prepared'].append(item)
            print('SAVED' if args.apply else 'READY', rel, flush=True)
        except Exception as exc:
            report['errors'].append({'rom': str(rel), 'error': str(exc)})
            print('ERROR', rel, ':', exc, file=sys.stderr, flush=True)
    report['previews'] = previews(report['prepared'], stage)
    (stage / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(f'\n{len(report["prepared"])} prepared, {len(report["skipped"])} existing, {len(report["errors"])} errors.')
    print('Preview, sources, labels and report:', stage)
    print('Destination:', dest, '(copied)' if args.apply else '(unchanged; use --apply to copy)')
    return 1 if report['errors'] else 0


if __name__ == '__main__':
    sys.exit(main())
