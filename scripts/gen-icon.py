"""Draws the app icon master (1024x1024 PNG): iron's periodic-table tile, "Fe".

    python scripts/gen-icon.py            # writes src-tauri/icons/app-icon.png
    npx tauri icon src-tauri/icons/app-icon.png   # makes every platform's sizes

Needs Pillow + NumPy and the Segoe UI fonts (Windows). The output is committed.
"""

from pathlib import Path

import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

S = 1024
OUT = Path(__file__).resolve().parent.parent / "src-tauri" / "icons" / "app-icon.png"
FONTS = Path("C:/Windows/Fonts")
rng = np.random.default_rng(26)

MARGIN = 56
RADIUS = 210


def rounded_mask(inset: int = 0) -> Image.Image:
    m = Image.new("L", (S, S), 0)
    ImageDraw.Draw(m).rounded_rectangle(
        (MARGIN + inset, MARGIN + inset, S - MARGIN - inset, S - MARGIN - inset),
        RADIUS - inset, fill=255)
    return m


def tile() -> Image.Image:
    y = np.linspace(0, 1, S)[:, None]
    x = np.linspace(0, 1, S)[None, :]
    # Dark iron: blue-grey at the top to near black at the bottom.
    top, bottom = np.array([70, 82, 96]), np.array([20, 24, 30])
    base = top * (1 - y[..., None]) + bottom * y[..., None]
    # A soft diagonal sheen, like light on polished metal.
    d = (x + y * 0.8)
    sheen = np.exp(-((d - 0.62) ** 2) / 0.02) * 26
    # Fine horizontal brushing.
    brush = rng.standard_normal((S, 1)) * 2.2 + rng.standard_normal((S, S)) * 1.2
    img = base + sheen[..., None] + brush[..., None]
    out = Image.fromarray(np.clip(img, 0, 255).astype(np.uint8), "RGB").convert("RGBA")
    out.putalpha(rounded_mask())
    return out


def bevel(img: Image.Image) -> None:
    """A thin light edge at the top, darker at the bottom: the tile has thickness."""
    edge = ImageChops.subtract(rounded_mask(), rounded_mask(10))
    y = np.linspace(1, 0, S)[:, None].repeat(S, 1)
    light = Image.fromarray((np.clip(y * 1.6 - 0.3, 0, 1) * 110).astype(np.uint8))
    hi = Image.new("RGBA", (S, S), (255, 255, 255, 0))
    hi.putalpha(ImageChops.multiply(edge, light))
    img.alpha_composite(hi)
    ring = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(ring).rounded_rectangle(
        (MARGIN + 1, MARGIN + 1, S - MARGIN - 1, S - MARGIN - 1), RADIUS, outline=(0, 0, 0, 150), width=3)
    img.alpha_composite(ring)


def metal_text(img: Image.Image, text: str, font: ImageFont.FreeTypeFont, center: tuple[int, int]) -> None:
    """Silver letters: a light-to-steel gradient, a soft drop shadow and a crisp top highlight."""
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).text(center, text, font=font, fill=255, anchor="mm")

    shadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    shadow.putalpha(mask.filter(ImageFilter.GaussianBlur(16)).point(lambda v: v * 0.75))
    img.alpha_composite(shadow, (0, 14))

    y = np.linspace(0, 1, S)[:, None].repeat(S, 1)
    grad = np.stack([255 - y * 95, 255 - y * 88, 255 - y * 78], -1)
    fill = Image.fromarray(np.clip(grad, 0, 255).astype(np.uint8), "RGB").convert("RGBA")
    fill.putalpha(mask)
    img.alpha_composite(fill)

    edge = ImageChops.subtract(mask, ImageChops.offset(mask, 0, 4))
    hi = Image.new("RGBA", (S, S), (255, 255, 255, 0))
    hi.putalpha(edge.point(lambda v: v * 0.8))
    img.alpha_composite(hi)


def main() -> None:
    img = tile()
    bevel(img)
    big = ImageFont.truetype(str(FONTS / "segoeuib.ttf"), 470)
    small = ImageFont.truetype(str(FONTS / "segoeuisl.ttf"), 92)
    metal_text(img, "Fe", big, (S // 2, S // 2 + 30))
    d = ImageDraw.Draw(img)
    d.text((MARGIN + 92, MARGIN + 74), "26", font=small, fill=(176, 188, 202, 255), anchor="lm")
    d.text((S // 2, S - MARGIN - 96), "55.845", font=ImageFont.truetype(str(FONTS / "segoeuisl.ttf"), 64),
           fill=(126, 138, 152, 255), anchor="mm")
    img.save(OUT)
    print(OUT)


if __name__ == "__main__":
    main()
