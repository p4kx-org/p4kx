# Version-154 node effect slots and Aux routing preserve original declarations

Version-154 Sound and ActorMixer share a node base. It starts with a parent
effect override byte and an unsigned byte count. A nonempty effect list adds a
bypass-all byte. Each slot contains an unsigned byte index, a little-endian
u32 effect reference and a byte of flags. The following metadata list begins
with its own parent override and count. Each metadata slot has an index, an
effect reference and a share-set byte.

The routing region follows positioning. Its flag byte has bit 3 set when four
user Aux bus words follow. A little-endian reflections Aux bus word follows
the optional list in either case. The reader preserves the full flag and slot
bytes and retains every complete ID word in its original physical slot. The
only slot-count bound is the original byte count. The number of optional Aux
bus words comes from the format.

The shared traversal records checked source extents. A slot cursor reads from
the original owning bank and stops at its recorded extent. A truncated slot
can supply its index and complete effect reference independently. A truncated
Aux list supplies its complete bus words in their original positions. Missing
regions, original zero counts and original zero IDs keep distinct meanings.
Opaque original bytes remain in the retained bank. The bulk records qualify
these declarations to version 154 and attach exact per-field source ranges.
Effect and bus IDs are authored references with unchecked target resolution.
The existing playback traversal consumes the same checked regions.

The layouts follow [wwiser's initial effect reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L344),
[wwiser's node metadata reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L973),
[wwiser's Aux reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L464)
and [StarBreaker's shared node-base reader](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/node_base.rs#L200).
