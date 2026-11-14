# Standalone State properties use a versioned planar bundle

The standalone HIRC type 1 reader supports bank versions 128 through 145. The body after the
object ID starts with a little-endian unsigned 16-bit count. All unsigned 16-bit RTPC parameter
IDs precede the same number of 32-bit IEEE 754 values. The encoded count and available body
bytes bound the reader before materialization. It preserves repeated IDs, unknown IDs, signed
zero and every nonfinite bit pattern.

Both slice and seek readers report bank-relative half-open byte ranges for the count, each ID
and value, and the consumed bundle. The standalone object exposes the remaining body range.
ObjectView keeps exact unsigned value bits and emits a numeric value only for finite words.

The version boundary follows wwiser's standalone State dispatch and its documented transition
to inline group values in v146. The same planar property bundle is used within v154 state groups.
Support for the standalone v154 layout requires separate evidence, so its original HIRC body
remains available with unsupported layout status. No installed-archive presence or coverage is
inferred from the synthetic fixtures.

[StarBreaker State dispatch](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/misc.rs),
[StarBreaker planar bundle](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/props.rs),
[wwiser standalone State](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L1019),
[wwiser state groups](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L513).

## Inline node State groups

The version-154 Sound node reader records the checked State extent before reading RTPCs.
The source-bound State cursor walks its binding and group counts, variable-length RTPC
property IDs, original flag bytes, ordered State IDs, and their planar value bundles.
Each State group and member keeps its physical ordinal even when IDs repeat. An explicit
empty chunk carries both original zero counts. A Sound with no node has an absent State chunk.
A format failure before the extent is established leaves the original body recoverable.
A later RTPC failure leaves the independently checked State extent available.

The node parser checks extents without materializing groups. The opt-in cursor holds one
active group position and one value bundle. Its source reads are confined to the checked
State extent, including when opaque or malformed bytes follow the State chunk.

[StarBreaker inline State parser](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/rtpc.rs#L52),
[wwiser v154 group layout](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L513).
