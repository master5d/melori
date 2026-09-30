"""Build the melori app and tray icons: royal-blue tile with a paper-coloured Literata «m» in outlines.

Reads Literata (variable) from node_modules/@fontsource-variable/literata, instantiates
wght=700 / opsz=72, converts the glyph «m» to an SVG path and writes design/icon/melori.svg
(plus design/icon/tray-*.svg for the three tray states). The SVGs have no <text> and no font
reference, so the icons never depend on installed fonts.

With --apply it also rasterises them through `tauri icon`: the app icon set lands in
src-tauri/icons/, the 32 px tray states in src-tauri/resources/tray_*.png (the tile is
opaque, so the light- and dark-taskbar variants are the same file).

Needs fontTools + brotli (woff2):  python scripts/build-icon.py [--apply]
"""
from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

ROOT = Path(__file__).resolve().parents[1]
FONT = ROOT / "node_modules/@fontsource-variable/literata/files/literata-latin-standard-normal.woff2"
OUT = ROOT / "design/icon"

SIZE = 1024
RADIUS = 208
TILE = "#2448B8"  # accent (design/tokens.json, day)
INK = "#F4EFE6"  # ground (design/tokens.json, day)
ERR = "#A3342A"  # err (design/tokens.json, day)
X_HEIGHT_SHARE = 0.46  # x-height of the letter as a share of the tile
MAX_WIDTH_SHARE = 0.64  # «m» is wide: cap its width so the tile keeps a margin (approved mockup ≈ 2/3)

# tray state marks, bottom-right corner below the baseline (clear of the letter); sized for 32 px
MARKS = {
    "idle": "",
    "recording": f'  <circle cx="850" cy="850" r="118" fill="{ERR}" stroke="{INK}" stroke-width="48"/>\n',
    # a solid paper dot: an open ring reads as the letter «c» at 32 px
    "transcribing": f'  <circle cx="850" cy="850" r="118" fill="{INK}"/>\n',
}


def glyph_path(font: TTFont, char: str) -> tuple[str, tuple[float, float, float, float]]:
    glyph_set = font.getGlyphSet()
    name = font.getBestCmap()[ord(char)]
    pen = SVGPathPen(glyph_set)
    glyph_set[name].draw(pen)
    bounds = BoundsPen(glyph_set)
    glyph_set[name].draw(bounds)
    return pen.getCommands(), bounds.bounds


def letter() -> str:
    font = instantiateVariableFont(TTFont(str(FONT)), {"wght": 700, "opsz": 72})
    path, (x_min, _, x_max, _) = glyph_path(font, "m")
    _, (_, _, _, xh) = glyph_path(font, "x")
    scale = min(SIZE * X_HEIGHT_SHARE / xh, SIZE * MAX_WIDTH_SHARE / (x_max - x_min))
    # font units are y-up, SVG is y-down: flip around the baseline; the x-height band
    # (baseline .. x-height) is centred on the tile
    tx = SIZE / 2 - (x_min + x_max) / 2 * scale
    ty = SIZE / 2 + xh * scale / 2
    return f'  <path transform="translate({tx:.2f} {ty:.2f}) scale({scale:.5f} {-scale:.5f})" fill="{INK}" d="{path}"/>\n'


def svg(body: str) -> str:
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SIZE} {SIZE}" width="{SIZE}" height="{SIZE}">\n'
        f'  <rect width="{SIZE}" height="{SIZE}" rx="{RADIUS}" fill="{TILE}"/>\n'
        f"{body}</svg>\n"
    )


def tauri_icon(source: Path, out_dir: Path) -> None:
    tauri = ROOT / "node_modules/.bin" / ("tauri.exe" if sys.platform == "win32" else "tauri")
    subprocess.run([str(tauri), "icon", str(source), "-o", str(out_dir)], check=True, cwd=ROOT)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--apply", action="store_true", help="rasterise into src-tauri/icons and tray resources")
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    m = letter()
    app_svg = OUT / "melori.svg"
    app_svg.write_text(svg(m), encoding="utf-8", newline="\n")
    tray_svgs = {}
    for state, mark in MARKS.items():
        tray_svgs[state] = OUT / f"tray-{state}.svg"
        tray_svgs[state].write_text(svg(m + mark), encoding="utf-8", newline="\n")
    print(app_svg)
    if not args.apply:
        return
    tauri_icon(app_svg, ROOT / "src-tauri/icons")
    resources = ROOT / "src-tauri/resources"
    for state, source in tray_svgs.items():
        with tempfile.TemporaryDirectory() as tmp:
            tauri_icon(source, Path(tmp))
            for name in (f"tray_{state}.png", f"tray_{state}_dark.png"):
                shutil.copyfile(Path(tmp) / "32x32.png", resources / name)
                print(resources / name)


if __name__ == "__main__":
    main()
