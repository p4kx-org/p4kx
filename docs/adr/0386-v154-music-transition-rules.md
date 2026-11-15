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
and [StarBreaker's transition rule reader](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/music.rs#L136).

The same pass records whether checked rules carry nonzero fade time or
offset words, and whether an allocated object names a nonzero segment.
Those observations follow the physical nodes reached by TXTP. A rule
count and an empty allocation carry no omission code by themselves.

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
