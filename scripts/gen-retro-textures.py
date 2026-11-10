"""Generates the Retro theme's worn-metal textures (seamless tiles) into src/assets/retro/.

    python scripts/gen-retro-textures.py

Everything is procedural (no photos), so there is no licence to worry about. Needs
Pillow and NumPy. The output is committed, so the app build doesn't need Python.
Change SEED to get a different (but equally worn) look.
"""

from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

SIZE = 512
SEED = 7
OUT = Path(__file__).resolve().parent.parent / "src" / "assets" / "retro"
rng = np.random.default_rng(SEED)


def noise(scale: float, power: float = 2.0) -> np.ndarray:
    """Seamless fractal noise in 0..1. Filtering in the frequency domain makes it tile."""
    white = rng.standard_normal((SIZE, SIZE))
    fy = np.fft.fftfreq(SIZE)[:, None]
    fx = np.fft.fftfreq(SIZE)[None, :]
    f = np.sqrt(fx * fx + fy * fy)
    f[0, 0] = 1
    spectrum = np.fft.fft2(white) / (f ** (power / 2)) * np.exp(-((f * scale) ** 2))
    n = np.real(np.fft.ifft2(spectrum))
    return (n - n.min()) / (n.max() - n.min())


def stretched(scale_x: float, scale_y: float) -> np.ndarray:
    """Seamless noise stretched along x (for brushed metal and drips)."""
    white = rng.standard_normal((SIZE, SIZE))
    fy = np.fft.fftfreq(SIZE)[:, None]
    fx = np.fft.fftfreq(SIZE)[None, :]
    f = np.sqrt((fx * scale_x) ** 2 + (fy * scale_y) ** 2)
    f[0, 0] = 1
    n = np.real(np.fft.ifft2(np.fft.fft2(white) / f))
    return (n - n.min()) / (n.max() - n.min())


def rgb(hex_: str) -> np.ndarray:
    return np.array([int(hex_[i : i + 2], 16) for i in (1, 3, 5)], dtype=float)


def mix(a: np.ndarray, b: np.ndarray, t: np.ndarray) -> np.ndarray:
    return a * (1 - t[..., None]) + b * t[..., None]


def smooth(x: np.ndarray, lo: float, hi: float) -> np.ndarray:
    t = np.clip((x - lo) / (hi - lo), 0, 1)
    return t * t * (3 - 2 * t)


def scratches(count: int, length: tuple[int, int], light: str, dark: str, alpha: int) -> Image.Image:
    """Thin scratches drawn with wrap-around, at 2x then scaled down for anti-aliasing."""
    big = SIZE * 2
    layer = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    for _ in range(count):
        x, y = rng.uniform(0, big, 2)
        angle = rng.normal(0.35, 0.9)  # mostly one direction, like real wear
        n = rng.integers(*length) * 2
        dx, dy = np.cos(angle) * n, np.sin(angle) * n
        width = int(rng.choice([1, 1, 2, 2, 3]))
        a = int(alpha * rng.uniform(0.4, 1))
        for ox in (-big, 0, big):
            for oy in (-big, 0, big):
                p0 = (x + ox, y + oy)
                p1 = (x + dx + ox, y + dy + oy)
                d.line([p0, p1], fill=(*rgb(dark).astype(int), a // 2), width=width + 2)
                d.line([p0, p1], fill=(*rgb(light).astype(int), a), width=width)
    return layer.resize((SIZE, SIZE), Image.LANCZOS)


def worn_paint(paint: str, under: str, rust_strength: float, scratch_light: str) -> Image.Image:
    base = rgb(paint)
    mottle = noise(6) * 0.5 + noise(30) * 0.5
    color = base[None, None, :] * (0.86 + 0.24 * mottle[..., None])

    # Chipped paint: bare metal / primer showing through, with a dark rim.
    chips_n = noise(14, 2.4) * 0.7 + noise(60) * 0.3
    chip = smooth(chips_n, 0.68, 0.72)
    rim = smooth(chips_n, 0.63, 0.68) - chip
    metal = rgb(under)[None, None, :] * (0.8 + 0.4 * noise(80)[..., None])
    color = mix(color, color * 0.55, np.clip(rim, 0, 1) * 0.8)
    color = mix(color, metal, chip)

    # Rust: blotches plus streaks running down.
    rust_n = noise(4, 2.6) * 0.6 + stretched(0.2, 2.5) * 0.4
    rust = smooth(rust_n, 0.58, 0.80) * rust_strength
    rust_col = mix(rgb("#5a2a12")[None, None, :].repeat(SIZE, 0).repeat(SIZE, 1),
                   rgb("#9a4b1c")[None, None, :].repeat(SIZE, 0).repeat(SIZE, 1), noise(40))
    color = mix(color, rust_col, rust * (0.55 + 0.45 * noise(50)))

    # Grime: dirt settling in, a little darker overall in patches.
    grime = smooth(noise(5), 0.45, 0.9)
    color = color * (1 - 0.22 * grime[..., None])

    # Fine grain.
    color = color + (rng.standard_normal((SIZE, SIZE, 1)) * 9)

    img = Image.fromarray(np.clip(color, 0, 255).astype(np.uint8), "RGB").convert("RGBA")
    img.alpha_composite(scratches(110, (10, 90), scratch_light, "#1a1a1a", 150))
    img.alpha_composite(scratches(700, (2, 12), scratch_light, "#1a1a1a", 90))
    return img.convert("RGB")


def brass() -> Image.Image:
    brushed = stretched(0.03, 1.6)
    base = mix(rgb("#7a5a26")[None, None, :].repeat(SIZE, 0).repeat(SIZE, 1),
               rgb("#d2ad62")[None, None, :].repeat(SIZE, 0).repeat(SIZE, 1),
               0.35 + 0.5 * brushed)
    # Patina and fingerprints of age.
    patina = smooth(noise(5, 2.4), 0.55, 0.9)
    base = mix(base, rgb("#3f3a22")[None, None, :].repeat(SIZE, 0).repeat(SIZE, 1), patina * 0.45)
    base = base * (0.9 + 0.2 * noise(40)[..., None])
    img = Image.fromarray(np.clip(base, 0, 255).astype(np.uint8), "RGB").convert("RGBA")
    img.alpha_composite(scratches(120, (4, 30), "#f3dea6", "#2a1f0c", 90))
    return img.convert("RGB")


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    textures = {
        "casing.webp": worn_paint("#4f575d", "#9aa0a3", 1.0, "#c3c8ca"),
        "panel.webp": worn_paint("#c49a33", "#45484a", 0.75, "#f3e6bb"),
        "brass.webp": brass(),
    }
    for name, img in textures.items():
        path = OUT / name
        img.save(path, "WEBP", quality=80, method=6)
        print(f"{path.relative_to(OUT.parent.parent.parent)}  {path.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
