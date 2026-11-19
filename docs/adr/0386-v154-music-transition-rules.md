# Version-154 music transitions expose authored rules and ID lists

Ordinary version-154 MusicSwitch and MusicPlaylist share a transition rule
table after the music node. Each rule contains counted source and destination
ID lists, a 21-byte source rule, a 26-byte destination rule and an allocation
byte. A nonzero allocation byte introduces a 30-byte transition object.

The source and destination rules preserve fade time, curve and signed offset
words along with synchronization, cue, jump, entry and playback controls.
The transition object preserves its segment ID, both fades and raw playback
flags. Signed values retain their exact original words and units.

The rule reader advances only through bytes in its owning HIRC body. The
authored counts and checked ranges remain separate. An incomplete table can
retain complete preceding rules and the checked prefix of its final rule,
including complete IDs in a truncated list. Original lists preserve order,
duplicate IDs, zero values and sentinel words. The Media records identify
their physical rule and list positions and carry exact per-field source uses.

The ordinary layout follows [wwiser's music transition reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L2532)
and StarBreaker's transition rule reader (`crates/starbreaker-wwise/src/hirc/music.rs:136` at `7d3f01e76307354bbc56a64a70eb5167b1189b64`).

The same pass records whether checked rules carry nonzero fade time or
offset words, an allocated object names a nonzero segment, or an original
selector or flag signals an omitted scheduling control. Those observations
follow the physical nodes reached by TXTP. The diagnostic cites the original
word at its stored width. The [wwiser selector table](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wdefs.py#L1374-L1385)
identifies the non-immediate sync range.

The CLI and MCP expose this table in the owning music node's
`source_fields.transition_rules`. Rule and ID rows share the source inventory
with the stingers and use the same checked bank cursor. Table coverage
describes only the transition bytes, independently of later owner fields.

Version-154 MusicSwitch also exposes its decision block under
`source_fields.decisions`. It shares the positioned reader with Media and
uses an independent cursor for original group, node and checked target rows.
Group and node ordinals are physical positions. Target ordinals count
individual routes in traversal order, so two routes to the same original
leaf stay distinct. Raw node words and argument types remain available even
when a route fails. Each target ID cites the leaf payload and the branch
selectors. These fields record the checked source graph without choosing
runtime conditions. A malformed object suffix leaves the checked decision block
decoded.

### Shared physical decision nodes

A MusicSwitch child interval refers to absolute physical ordinals. Paths may
share a child or a subtree. The complete structure is checked from the root
using the earliest reachable argument level of each physical node. That is
sufficient to check whether its packed word can act as a branch. Out-of-range
intervals and cycles among these branches report a malformed graph. The
physical node rows preserve even invalid or unreachable source words.

The standard projection contains the whole physical graph. The ordered target
rows and the eager ID list are derived projections. Their replay uses four
work units per reachable physical node and two per authored argument up to the
number of reachable nodes. This counts the original scalar words. One visit
consumes one unit, including visits that end in a zero or a dead branch. A
target row with per-ancestor source evidence also consumes one unit per
ancestor. The ID list has no ancestor cost. A projection that would exceed
its input-proportional replay allowance reports `targets_compact: true`.
Unreachable physical nodes have no effect on this decision.

Structure validation covers later reachable branches even when a preceding
subtree would expand beyond the allowance. Expanded output retains the checked
target prefix before the first malformed visit. Compact output carries the
structure and malformed status without expanded target rows. From the graph,
a consumer can walk from ordinal zero, following the low-word child start and
high-word child count at each argument level. At the final level the whole word
is the target ID. This permits exact traversal without any fixed path-count
quota in the source representation.
