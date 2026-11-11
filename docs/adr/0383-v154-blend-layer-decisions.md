# Version-154 Blend containers expose authored layers and crossfade graphs

Ordinary HIRC type 9 in version 154 stores a 32-bit Layer count after the
shared node and child list. Each Layer stores its original ID, initial
RTPC curves, crossfade RTPC ID and type, and a 32-bit association count.
Each association stores a child ID and a 32-bit point count. The point
words carry two binary32 values and an original interpolation code.
A continuous-validation byte follows the Layer table.

The readers stream each nested table from a checked range in its owning
physical bank. Counts remain original values. Independent table coverage
identifies readable prefixes. An incomplete point can retain its input
and output words independently. A later malformed association leaves
an earlier complete initial RTPC region usable. Raw graph words, unknown
codes, repeated IDs, explicit zeroes and physical ordinals remain present.

The Media export uses dedicated Blend decision namespaces and the shared
initial RTPC payloads under Layer-specific namespaces. Numeric conversions
and each physical selector retain their field origins. Authored association
references carry unchecked resolution and remain independent from the original
child list and from any evaluated playback.

The ordinary layout follows [wwiser's Layer reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L2020)
and [StarBreaker's Blend reader](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/containers.rs#L193).
The raw marked custom bank versions fall outside the ordinary version gate. [wwiser custom version table](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wdefs.py).
