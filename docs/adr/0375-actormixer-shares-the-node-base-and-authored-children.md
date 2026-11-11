# ActorMixer shares the version-154 node base and retains its authored child order

The HIRC type-7 body in bank version 154 begins with the object ID and the
shared node base. A little-endian u32 child count follows the node, then exactly
that many little-endian u32 child IDs. IDs may repeat. The complete layout ends
with the last child, including a zero-length list. The reader treats additional
bytes as malformed and keeps the original body available.

The seekable reader exposes the node fields, checked State and RTPC spans and
ordered child words from the same physical occurrence. If the child count exceeds
the body, every complete child word before the cut remains available and the
object is malformed. A malformed RTPC retains any earlier complete State chunk
and checked curve prefix. A body that ends before the child count provides no
child declaration. Other bank versions retain the body as unsupported.

The slice reader exposes the parent and children through ObjectView along with
the half-open bank-relative node, count and child-list ranges. Existing playback
resolution treats ActorMixer as unsupported.

The layout follows [wwiser's ActorMixer dispatch and node order](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L1661),
[wwiser's child list](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L502)
and [StarBreaker's ActorMixer reader](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/containers.rs#L150).
