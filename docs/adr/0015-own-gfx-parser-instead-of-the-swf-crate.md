# Own GFx and SWF parser instead of the `swf` crate

UI movies in the Archive are Scaleform GFx files (`GFX`, `CFX`) and plain SWF files (`FWS`,
`CWS`, `ZWS`). The only maintained Rust SWF parser is Ruffle's `swf` crate (0.3.0, MIT OR
Apache-2.0); `swf-parser` (open-flash) is AGPL-3.0 and unmaintained since 2022. `swf` accepts the
GFx signatures, but returns the GFx tags (1000–1009: `ExporterInfo`, `DefineExternalImage2`,
`DefineCompactedFont`, …) as unknown bytes, decodes no bitmap, and reads a sprite's tags by
recursing into nested sprites without a depth limit, so a hostile movie can overflow the stack,
which aborts the process. We would parse the GFx tags, decode the bitmaps and convert its types
into our own anyway, and could not bound the nesting (ADR-0001's reasoning for the core formats
applies). `p4kx-gfx` therefore parses the movie itself, on `p4kx-foundation`, with every length
bounded by the data left and the limits `MAX_MOVIE_LEN`, `MAX_BITMAP_PIXELS` (32 Mi pixels; the
largest UI texture of the LIVE Archive has 22.3 Mi) and `MAX_SHAPE_RECORDS`.
Sprite definitions are walked with an explicit stack. Scene composition bounds nested
placements with `MAX_SPRITE_DEPTH`.
An extraction decodes its bitmaps in parallel within a budget of twice `MAX_BITMAP_PIXELS`.

It decompresses with `flate2` and `lzma-rs` (MIT) and decodes embedded PNGs with `png`. `lzma-rs`
has had no release since 0.3.0 in January 2023; it is small, pure Rust, used by `swf` and `zip`,
and only runs with the unpacked size the header declares and a memory limit, so a dormant release
line is acceptable here; `lzma-rust2` is the fallback should it break. Embedded JPEGs are decoded
with `zune-jpeg` (MIT OR Apache-2.0 OR Zlib), the pure-Rust decoder the `image` crate uses. Its
0.5.15 release is known to panic on crafted input in the progressive, IDCT and upsampling paths
(zune-image issue #409, fixed on the development line towards 0.5.16), so every call into it
runs inside `catch_unwind` and a panic becomes `GfxError::Jpeg`; `cargo xtask lints` keeps
`panic = "unwind"` in every profile for this, as for lewton (ADR-0011, ADR-0013). GIF bitmaps
are reported as not supported. Ruffle's handling of SWF quirks (the `FF D9 FF D8` marker pairs
in JPEG data, premultiplied colours in `DefineBitsLossless2` and `DefineBitsJPEG3`) and the
StarBreaker `gfx` crate (MIT) serve as references.

External images of a movie on disk are read only as DDS files inside the nearest ancestor folder
named `Data`, or else the movie's own folder: the path is normalised lexically, checked against
that root, and checked again after canonicalisation, so a movie cannot make an extraction read
files elsewhere on the disk.

## Considered Options

- The `swf` crate: fastest start for the plain SWF tags, but the GFx tags, the bitmap decoding,
  the bound on nesting and the conversion into our typed values stay ours, and its dependencies
  (`bitstream-io`, `encoding_rs`, `enum-map`, `num-derive`, `simple_asn1`, …) come along.
- Porting StarBreaker's `gfx` crate: it reads only the tag summary and the first-frame placements,
  not shapes, bitmaps or fonts.
- `jpeg-decoder` instead of `zune-jpeg`: the `image` project replaced it with `zune-jpeg`.
