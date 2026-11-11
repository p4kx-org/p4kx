# Version-154 MusicTrack exposes authored automation and Switch transitions

The version-154 track layout carries a flags byte after its source table,
an original subtrack count when clips are present, and a counted automation
table before the shared node base. Each automation occurrence stores a clip
index, a raw automation type and a counted graph. Its point words contain
binary32 input and output and the original interpolation code.

A Switch track contains eight transition words after its original member
list. Source and destination fades each have a signed time, a curve code
and a signed offset. The two fades surround a synchronization code and
cue filter hash. Every track ends with a signed look-ahead word.

The Media projection adds three physical namespaces for track controls,
automation and points. It retains explicit zeroes, repeated clip indices,
unknown codes, original numeric bits and available numeric interpretations
in their original units. Checked ranges and independent table coverage
preserve readable prefixes. Existing source, clip and switch member rows
retain their prior meaning. A clip index remains an authored declaration.

The MusicTrack and Switch transition layouts follow [wwiser's MusicTrack reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L2225),
[wwiser's transition reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L2183)
and [StarBreaker's MusicTrack reader](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/music.rs#L341).
