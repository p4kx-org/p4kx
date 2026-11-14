# Version-154 music stingers expose authored scheduling words

MusicSegment, MusicSwitch and MusicPlaylist use the same version-154 music
node stinger table. Each stinger carries a trigger ID, segment ID,
synchronization code, cue filter hash, signed repeat-time word and segment
look-ahead count. All fields occupy one original 32-bit word each.

The bank reader preserves the four scheduling words and their exact byte
ranges in addition to the existing stinger IDs and opaque range. Complete
records retain their original order. A truncated final row with a readable
trigger ID retains its checked prefix and ordinal. The retained raw object
covers any incomplete word.

The Media export uses one physical schedule namespace for all three owning
music object kinds. Each field carries the original bank source, and the
signed repeat time also retains its raw bits. Explicit zeros, repeated IDs
and unknown synchronization codes remain present. The exported fields are
authored scheduling declarations.

The version-154 word order and signed repeat time follow [wwiser's music node reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L2116)
and [StarBreaker's music node reader](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/music.rs#L56).
