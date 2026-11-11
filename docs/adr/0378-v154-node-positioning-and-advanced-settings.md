# Version-154 positioning and advanced settings retain original node fields

The version-154 shared node positions the original Positioning region after
the two property tables. Bit 0 of the positioning byte selects the parent
override and bit 1 contributes to the presence of the following 3D flags.
Bits 2–3 carry the panner code. Bits 5–6 carry the 3D position code. When the
3D byte is present and the position code is nonzero, automation follows.

The automation region starts with an unsigned byte path mode and signed
little-endian 32-bit transition time. Its unsigned 32-bit vertex count
precedes ordered XYZ binary32 words and a signed 32-bit duration per vertex.
An unsigned 32-bit playlist count precedes unsigned 32-bit vertex offsets and
counts. A separate table then supplies one XYZ binary32 range row per playlist
item. The reader retains the original offset word and signed times in their
source representation. Media provides finite coordinate values separately
from the original words. NaN, infinity and signed zero remain recoverable
from their original bits.

The physical object bounds govern each table. The scanner uses the full
unsigned count and checks the available bytes with 64-bit arithmetic. The
source-bound cursor preserves every complete scalar in the checked prefix,
including fields of a partially readable final row. A zero count establishes
an empty authored table. A short field supplies no invented value, and the
original bank retains its exact bytes.

AdvancedSettings follows auxiliary routing. Its byte order is a flag byte, a
virtual-queue behavior byte, a little-endian unsigned 16-bit instance limit,
a below-threshold behavior byte and a second flag byte. Each independently
readable field is available before a later error. Original modes and flag
bytes retain unknown codes and bits. Both regions retain exact bank-relative
evidence for each value and for each finite conversion.

The verified field order comes from [wwiser's Positioning reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L782),
[wwiser's AdvancedSettings reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L289)
and [StarBreaker's shared node reader](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/node_base.rs#L70).
