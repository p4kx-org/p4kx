# Version-154 DialogueEvent exposes original decision words

An ordinary version-154 DialogueEvent body begins with an event probability
byte and an unsigned argument count. The count locates a planar table of
32-bit argument IDs followed by a table of argument type bytes. A 32-bit tree
byte count and a decision mode byte precede the tree. The declared tree
boundary locates the scalar and ranged property bundles. The original counts
and checked physical prefixes remain separate throughout export.

Each ordinary physical tree node contains a 32-bit key, a 32-bit link word and
two 16-bit words for weight and probability. The link word can represent a
target or packed child coordinates. Early leaves make classification by
declared depth alone insufficient. Media records preserve the original link
as undetermined and retain physical order and repeated words. The raw HIRC
envelope still locates the owning bytes. The bounded source cursor streams
the available fields without allocating storage from the declared counts.

A truncated tree preserves complete physical nodes and any readable words
of its final node. If all declared tree bytes are present, property parsing
can use the declared boundary even when the node table ends on a partial
row. Known decision modes and argument types have independent source-backed
interpretations. Unrecognized codes remain raw and explicit. Existing
eager Soundbank playback and extraction use their existing layout coverage.

The ordinary field order is established by
[wwiser's DialogueEvent and decision tree readers](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L2836)
and [StarBreaker's DialogueEvent reader](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/misc.rs#L43).
The supported mode and group labels follow
[wwiser's enum table](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wdefs.py#L1356).
