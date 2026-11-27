# The P4KX logo

Data brackets around an X, in theme colours only (ADR-0075). The drawing is the reference; scale
it, never redraw it.

Files: [`p4kx-logo.svg`](p4kx-logo.svg) (tile), [`p4kx-mark.svg`](p4kx-mark.svg) (mark),
[`p4kx-logo.png`](p4kx-logo.png) (tile, 512 px), [`p4kx-social-preview.png`](p4kx-social-preview.png)
(1280 x 640, for the repository settings) and `crates/p4kx-gui/assets/p4kx.ico` (tile at 16, 24,
32, 48, 64 and 256 px, embedded in `p4kx-gui.exe`, `p4kx.exe` and the installer).

## Geometry

A 130 x 130 unit square. `p4kx_gui::theme::logo` rasterises exactly this.

| Part | Drawing | Colour |
| --- | --- | --- |
| Tile | rounded square, `rx` 24 | `BRAND_TILE` `#0F0F0F` |
| Left bracket | `M42,30 H28 V100 H42`, stroke 9, mitred joins, no fill | `WHITE` `#FFFFFF` |
| Right bracket | `M88,30 H102 V100 H88`, stroke 9, mitred joins, no fill | `WHITE` `#FFFFFF` |
| X, back stroke | line (50,42) to (80,88), stroke 11, round caps | `ACCENT_DARK` `#C45C00` |
| X, front stroke | line (80,42) to (50,88), stroke 11, round caps, on top | `PRIMARY` `#E77E23` |

The mark is the same drawing without the tile, on a transparent ground. The tile is the icon.

## Use

- **Tile:** window and taskbar icon, executables, installer, the README, avatars. It keeps the
  logo legible on a light or a dark ground.
- **Mark:** the header (32 px, before the wordmark with an 8 px gap), on dark grounds.
- **Minimum size:** tile 16 px, mark 24 px.
- **Clear space:** at least the length of one bracket arm (14 units, about 11% of the side) on
  every side of the drawing.

## Do not

- Recolour any stroke, or use a colour that is not a `p4kx_gui::theme::palette` constant.
- Put the mark on a light ground; the white brackets vanish.
- Stretch, rotate, outline, shadow or animate the logo, or change the stroke widths.
- Swap the two strokes of the X: the orange one is in front.
- Draw the tile without its rounded corners.

## Regenerating

The `.ico` and the PNG files are generated from the geometry and committed:

```powershell
$env:P4KX_WRITE_LOGO_ASSETS = '1'
cargo test -p p4kx-gui --test it logo::
```

The test writes the files and then checks the `.ico` and the 512 px PNG against the geometry. The
social preview needs a graphics adapter (a software one is enough) and is only checked for its
size and ground, not pixel for pixel. The SVG files are edited by hand; a test builds the expected
paths and strokes from the constants of `p4kx_gui::theme::logo` and compares them, and the colours,
with the files.
