# DDS source-header metadata

`p4kx texture info --json` adds `source_metadata` without changing its existing fields or human output.
The projection has `schema_version:1`, `header_source_bytes`, `raw`, `normalized`, `transfer` and `alpha_association`.
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

## DDS transfer declarations and their evidence

The `transfer` object retains the exact Cry image flags and marker words, the
DXGI and Cry interpretations, the effective transfer, and whether the known
declarations conflict. Older metadata without this object remains readable
and provides no transfer proof.

An explicit typed sRGB or linear DXGI format is independent of the legacy
Cry declaration. The Cry marker is the final legacy word at body byte 120.
The image flags word is at body byte 32. Both offsets shift by four when
DDS magic is present. Only the recognised Cry marker makes bit `0x800`
meaningful. An absent marker and a legacy pixel format leave the transfer
unknown. Two opposing known declarations retain their individual values,
set the conflict field, and leave the effective transfer unknown.

The DDS source transfer is independent of the texture's material role and
the output file's transfer.

See the [DXGI format definitions](https://learn.microsoft.com/en-us/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format)
and [Cry image extension header](https://github.com/aws/lumberyard/blob/master/dev/Code/CryEngine/CryCommon/ImageExtensionHelper.h).

## DDS alpha declarations, coverage and output

`alpha_association` is the source header's claim about the fourth channel. Legacy
`DXT2` and `DXT4` declare premultiplied colour, whereas `DXT3` and `DXT5` declare
straight colour. DX10 uses the low three bits of `misc_flags2`. Values 0 through 4
mean unknown, straight, premultiplied, opaque and custom data. Values 5 through 7
are reserved. The original word, including all upper bits, remains available in
`raw.dx10.misc_flags2`. Older metadata can omit the association.

`Mip::alpha_coverage` measures the original BC1, BC2 or BC3 block values for one
selected image. The counts `zero`, `partial` and `full` include only in-bounds texels.
BC1 alpha is zero only where a transparent selector is actually used. Separate
array elements, faces and volume slices are measured independently. Coverage
does not decide opacity, a material blend mode or packed smoothness.

PNG and glTF colour use straight opacity. For premultiplied colour with a known
transfer, RGB is divided by the original alpha in linear light and then encoded
for the output transfer. With an unknown transfer, division uses the stored
sample values and PNG stays untagged. Zero original alpha yields black. A
separate alpha tail supplies the output alpha after RGB has been straightened
using the embedded alpha. The explicit role of a data channel leaves its RGB
and alpha processing independent of this colour operation.

`p4kx texture png --json` and converted bulk texture reports carry
`alpha_association` and an optional `alpha_transform` with exact original header
origins. Media image records additionally expose `alpha_coverage` when supported.
The PNG and GLB artifact origins include the original selected surface bytes.
The raw DDS writer preserves the source blocks.

See [DDS alpha modes](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header-dxt10),
[PNG alpha processing](https://www.w3.org/TR/png-3/#12Alpha-channel-processing)
and [glTF material textures](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#metallic-roughness-material).
