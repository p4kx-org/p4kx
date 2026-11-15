# Original soundbank attenuation

The audio reader admits HIRC type 14 in bank version 154. The body starts with the
object ID. The original height-spread flag byte precedes the cone flag byte.
Cone flag bit zero controls the presence of five original IEEE-754 words for the
inside and outside angles in degrees, outside volume, low-pass and high-pass
values. The export preserves the original bytes and uses documented labels for
the unconverted volume and filter words.

The next 19 signed bytes are the curve selectors in physical slot order.
Negative selectors retain their original numbers. The conversion curves carry
their original scale code and source point count. Each point preserves its
input and output bits and its full interpolation word. The shared initial RTPC
reader preserves the following count, curves and points independently from the
conversion curves. The exporter keeps source order and infers no acoustic
behaviour from these fields.

`bnk objects --json` and the MCP bank object projection use `source_fields` for
the original fields, ordered selector/curve/point rows, `initial_rtpc` and their
source inventory. Each sourced field includes its own provenance. Other
versions report an unsupported layout, with the original object extent.

Media export writes `audio-bank-hirc-attenuation` summaries and separate
`-selector`, `-curve`, `-point`, `-rtpc`, `-rtpc-curve` and `-rtpc-point`
rows. They join by the physical source and the HIRC section and object indices.
Conversion points add the curve and point indexes. Raw, optional decimal and
source values remain distinct. Every row has field-addressed byte receipts.
The raw HIRC object and bank artifacts retain all original bytes, including any
unprojected suffix or unsupported-version body.

The v154 original-source check selected three CRC-verified banks per installed
Build. In LIVE 13165946 and PTU 13189690, all 32 selected type-14 bodies per
Build fit the version-154 height-spread and cone prefix through their exact end.
They carry 163 conversion curves and 533 points per Build. Synthetic fixtures
cover enabled cones, nonempty initial RTPCs and physical failures. These
selected installed banks contained no enabled cones or nonempty initial RTPCs.

[ADR-0250](adr/0250-bnk-objects-prints-the-hirc-hierarchy-from-the-existing-decoding-and-counts-what-it-cannot-read.md)
