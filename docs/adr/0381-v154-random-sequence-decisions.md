# Version-154 Random/Sequence containers expose authored decisions

HIRC type 5 in version 154 stores three loop halfwords, three binary32
transition words, an avoid-repeat halfword, three mode bytes and a flags
byte after the shared node base. The original child list is followed by an
unsigned 16-bit playlist count and an ordered table of 32-bit play IDs and
signed 32-bit weights. The original child and playlist orders are independent.
The original bytes may carry repeated IDs and signed weights.

The bank-bound reader keeps checked source ranges for each field. The playlist
cursor emits each item whose play ID fits, including a final item with a
truncated weight. The count is bounded by the original record and its
format-defined width. The Media projection retains explicit zeroes, unknown
codes and flags, the original transition and weight words, and finite
binary32 and signed i32 interpretations where available. Each original value
and conversion has an exact receipt from the owning bank.

The Media decision summary and playlist rows use separate physical namespaces.
An unsupported layout, an unreachable decision region, and a readable partial
region have distinct summary layouts. Raw HIRC and existing child and mode
inspection remain available. Playlist references are authored IDs with
unchecked target resolution. These records carry original numeric values
without converting time units, assigning special loop-count behavior, or
representing selection as playback.

The v154 layout and signed weights are established by the version branches in
[wwiser's Random/Sequence reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L1501)
and [StarBreaker's container reader](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/containers.rs#L33).
