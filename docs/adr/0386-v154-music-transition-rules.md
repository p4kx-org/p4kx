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
