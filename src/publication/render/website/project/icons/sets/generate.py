#!/usr/bin/env python3
"""Regenerate icons.bin from extracted upstream npm packages.

Usage: generate.py MDI_DIR OCTICONS_DIR SIMPLE_ICONS_DIR FONTAWESOME_DIR OUT

Each *_DIR is the `package` directory of the pinned tarball named in
README.md. The script never executes upstream code: it reads SVG files,
keeps only <path> geometry and the root viewBox, and writes a sharded,
deflate-compressed bundle.

Bundle layout: b"GAICONS1\\n", then one index line per shard
"<shard>\\t<offset>\\t<length>\\n", then "\\n", then the raw-deflate shards
concatenated. Offsets are relative to the first shard byte. A shard is the
sorted lines "<shortcode>\\t<viewBox>\\t<paths>\\n" whose shortcode starts
with the shard key: the set prefix plus the first name character.
"""
import os
import re
import sys
import zlib

PATH = re.compile(r'<path\s+([^>]*?)\s*/?>(?:</path>)?')
ATTRIBUTE = re.compile(r'([a-z-]+)="([^"]*)"')
KEPT = ('d', 'fill-rule', 'clip-rule')
VIEWBOX = re.compile(r'^[0-9. -]+$')
DATA = re.compile(r'^[0-9A-Za-z.,\s+-]+$')


def icon(path):
    with open(path, encoding='utf-8') as handle:
        text = handle.read()
    text = re.sub(r'<!--.*?-->', '', text, flags=re.S)
    text = re.sub(r'<title>.*?</title>', '', text, flags=re.S)
    root = re.match(r'\s*<svg([^>]*)>(.*)</svg>\s*$', text, re.S)
    if not root:
        raise SystemExit(f'{path}: not a single SVG root')
    view_box = dict(ATTRIBUTE.findall(root.group(1).replace('viewBox', 'viewbox')))
    view_box = view_box.get('viewbox', '')
    if not VIEWBOX.match(view_box):
        raise SystemExit(f'{path}: unexpected viewBox {view_box!r}')
    body = root.group(2).strip()
    paths = []
    for match in PATH.finditer(body):
        attributes = dict(ATTRIBUTE.findall(match.group(1)))
        if attributes.get('fill') == 'none':
            continue
        if not DATA.match(attributes.get('d', '')):
            raise SystemExit(f'{path}: unexpected path data')
        kept = ''.join(
            f' {name}="{attributes[name]}"' for name in KEPT if name in attributes
        )
        paths.append(f'<path{kept}/>')
    if PATH.sub('', body).strip() or not paths:
        raise SystemExit(f'{path}: elements other than <path>')
    return view_box, ''.join(paths)


def collect(directory, prefix, strip=''):
    for name in sorted(os.listdir(directory)):
        if name.endswith('.svg'):
            stem = name[:-4]
            if strip and stem.endswith(strip):
                continue
            yield prefix + stem, *icon(os.path.join(directory, name))


def main(mdi, octicons, simple, fontawesome, out):
    sources = [
        ('material-', os.path.join(mdi, 'svg')),
        ('octicons-', os.path.join(octicons, 'build', 'svg')),
        ('simple-', os.path.join(simple, 'icons')),
    ] + [
        (f'fontawesome-{style}-', os.path.join(fontawesome, 'svgs', style))
        for style in ('brands', 'regular', 'solid')
    ]
    shards = {}
    for prefix, directory in sources:
        for shortcode, view_box, paths in collect(directory, prefix):
            if not re.match(r'^[a-z0-9_-]+$', shortcode):
                raise SystemExit(f'unexpected icon name {shortcode!r}')
            key = shortcode[: len(prefix) + 1]
            shards.setdefault(key, []).append(f'{shortcode}\t{view_box}\t{paths}\n')
    index, blobs, offset = [], [], 0
    for key in sorted(shards):
        compressor = zlib.compressobj(9, zlib.DEFLATED, -15)
        data = ''.join(sorted(shards[key])).encode('utf-8')
        blob = compressor.compress(data) + compressor.flush()
        index.append(f'{key}\t{offset}\t{len(blob)}\n')
        blobs.append(blob)
        offset += len(blob)
    with open(out, 'wb') as handle:
        handle.write(b'GAICONS1\n' + ''.join(index).encode() + b'\n')
        for blob in blobs:
            handle.write(blob)


if __name__ == '__main__':
    if len(sys.argv) != 6:
        raise SystemExit(__doc__)
    main(*sys.argv[1:])
