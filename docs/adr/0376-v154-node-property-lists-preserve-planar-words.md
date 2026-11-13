# Version-154 node property lists preserve planar IDs and raw words

The shared Wwise NodeBase layout in version 154 stores a packed priority and MIDI
behavior byte immediately after the direct parent word. The export retains its
original value and one-byte source span, including unassigned bits and explicit
zero. Two property lists follow before positioning. Each begins with an unsigned
byte count and a planar array of that
many unsigned byte IDs. The property list then stores one little-endian u32 word
per ID. The ranged modifier list stores a little-endian minimum and maximum u32
pair per ID. IDs and words retain their original order, including repeated IDs,
unknown codes, zero, signed zero and nonfinite binary32 encodings.

The original words use the authored property vocabulary. A property ID may name
a reference or another nonfloat representation. The reader exposes bits and
original byte ranges for every checked word. It leaves numeric and reference
interpretation to independently qualified property mappings.

A truncated ID array provides its readable prefix and no word position. A
truncated word array retains every complete word, including a ranged minimum
whose maximum is incomplete. A readable zero count yields an empty complete
list. An unreadable count supplies no list. The original object keeps its
malformed disposition and raw body. Its earlier override bus and parent words
remain available when their original reads succeeded, including an explicit
zero in either field.

Sound and ActorMixer provide fixed-memory property cursors bound to the
original bank. The shared layout traversal retains the same bounds and resolver
behavior for container and music objects. The Action property format and
bulk Media field groups have separate coverage.

The field ordering follows [wwiser's property bundle](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L108),
[wwiser's ranged bundle](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L148),
[wwiser's original bus and parent words](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L706)
and [StarBreaker's planar property readers](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/props.rs#L21).
