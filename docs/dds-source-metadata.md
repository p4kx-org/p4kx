# DDS source-header metadata

`p4kx texture info --json` adds `source_metadata` without changing its existing fields or human output.
The projection has `schema_version:1`, `header_source_bytes`, `raw`, and `normalized`.
`Texture::source_metadata` returns the same Extraction DTO without another source read.

`header_source_bytes` is the header Entry's declared decoded length or disk file length, excluding
separate numbered parts and alpha-tail files. It is a canonical unsigned 64-bit decimal JSON string.
It is not compressed Archive storage size, the whole sibling set's size, or an integrity proof.

`raw.magic_present` records whether the four DDS magic bytes were present; `header_length` includes
those bytes and a declared DX10 extension. `raw.legacy` contains all 31 little-endian words: size,
flags, height, width, pitch/linear size, depth, mip count, eleven reserved words, all eight pixel-format
words, caps1..4 and reserved2. FourCC is exactly four unsigned bytes, including nontext codes. Other
words are unsigned 32-bit JSON integers; unknown bit patterns are preserved. `raw.dx10` is present and
null for a legacy header, otherwise it contains all five unmodified extension words including
`misc_flags2`. No engine meaning or validation failure is invented from unused/reserved words.

`normalized` retains the existing reader's format label, dimensions, mip count, array size, face
count and cubemap predicate, plus its known encoded block layout and high-range classification.
Zero source mip/array counts and unused depth can therefore remain zero in `raw` while their reader
values are `1`. Layout is explicitly null when unknown. A known layout or high-range classification
is independent of general decoder availability, image conversion, and material-specific opacity
versus smoothness use. Raw legacy face bits remain available even when normalized face counts alone
do not identify individual orientations.

`DdsRawHeader::parse` captures structurally complete known-profile headers without topology defaults.
`DdsHeader::from_raw` performs the existing normalization and validation. Raw fields remain separately
projectable when topology is invalid. An incomplete header or unsupported structure size still
returns the original typed reader error; this DTO does not turn partial bytes into a complete header.

This header projection does not itself add the media TextureProvider, sibling graph, converted-image
records or package source-texture links. The original DDS merge continues to keep authored header
words; this metadata makes them machine-readable without changing its conversion behavior.

The word names and layouts follow Microsoft's [DDS_HEADER](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header),
[DDS_PIXELFORMAT](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-pixelformat) and
[DDS_HEADER_DXT10](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header-dxt10) declarations.
The implementation reads bytes through the existing portable reader and adds no Windows API dependency.
