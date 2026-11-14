# Sound RTPC retains the checked curve prefix

The version-154 Sound node stores initial RTPC curves after its inline State region.
An unsigned little-endian 16-bit curve count is followed by original-order curves.
Each curve contains a 32-bit RTPC ID, type and accumulation bytes, a Wwise variable-length
parameter ID, a 32-bit curve ID, a scaling byte and a 16-bit point count. Each point has
three little-endian 32-bit words. The first two are IEEE-754 input and output words,
and the last is the original interpolation code.

The bounded node scanner advances the checked RTPC range only after the complete point
extent of a curve fits. It keeps the declared count and every complete earlier curve when
a later header or point extent is malformed. The State range is checked independently before
RTPC traversal. A malformed RTPC therefore leaves the earlier State data available.
A Sound with no node has no RTPC chunk. A present zero count is an observed empty chunk.

The opt-in Sound RTPC cursor returns original-order curve and point rows without allocating
lists proportional to encoded counts. The physical body, the encoded counts and the checked
curve prefix bound its reads. Unknown flag and interpolation codes, repeated IDs, negative
zero and nonfinite binary32 words keep their original values and half-open bank-relative ranges.
The typed cursor admits the version-154 Sound node. Other versions retain the existing
opaque layout coverage.

The layout follows [StarBreaker's initial RTPC reader](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/rtpc.rs#L30),
[wwiser's RTPC curve parser](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L606)
and [wwiser's graph-point layout](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L58).
