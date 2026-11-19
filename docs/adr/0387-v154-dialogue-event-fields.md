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
declared depth alone insufficient. Media and ObjectView preserve physical
order, repeated words and the undetermined link interpretation. For each
readable link word, the low and high halves supply the possible first child
and count. The declared whole-node count bounds that window. Its intersection
with the readable whole-node prefix supplies the admitted count, even when
the window extends outside the declared table. A zero count within the table
is empty. An in-bounds nonempty window may still be truncated in the readable
prefix. A nonzero whole word is also a possible target ID. Both possibilities
may coexist at the same physical node, including a self-referencing window.

Ordinal zero is the root position. Elsewhere a zero key is a default candidate
and a nonzero key is a value candidate. On a proven route, a node at depth
`d > 0` uses argument `d - 1` in the planar table. Physical positions may
be shared by different routes. The exported windows and argument table
preserve the route information without assigning an argument to an unproven
route. The raw HIRC envelope locates the owning bytes. The source cursor
streams the admitted rows without allocating storage from the declared counts.

A truncated tree preserves complete physical nodes and any readable words
of its final node. If all declared tree bytes are present, property parsing
can use the declared boundary even when the node table ends on a partial
row. Known decision modes and argument types have independent source-backed
interpretations. Unrecognized codes remain raw and explicit. Existing
eager Soundbank playback and extraction use their existing layout coverage.

The ordinary field order is established by
[wwiser's DialogueEvent and decision tree readers](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L2836)
and StarBreaker's DialogueEvent reader (`crates/starbreaker-wwise/src/hirc/misc.rs:43` at `7d3f01e76307354bbc56a64a70eb5167b1189b64`).
The supported mode and group labels follow
[wwiser's enum table](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wdefs.py#L1356).
The child-window and candidate key interpretations follow
[wwiser's decision tree reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L2344-L2374)
and its [argument matching](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/generator/render/bnode_tree.py#L89-L165).


The CLI and MCP ObjectView share a source-fields projection from the original
opened bank. A checked physical cursor reads the selected object's own
envelope and verifies it against the parsed bank directory. The projection
streams original argument, node and property rows with direct per-value
receipts. Its original source coverage is separate from eager playback
coverage. The MCP response budget admits whole objects and explicitly
refuses an oversized object. The CLI can stream all checked rows for that
object. A standalone bank read has unknown original Archive encryption
lineage even when its physical file coordinates are known.
