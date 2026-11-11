# Soundbank metadata in the media export

`p4kx media export ARCHIVE --out DIR --kind audio --filter '*.bnk'` retains each selected
BNK Entry and streams its original physical directory into `audio.jsonl`. The physical source
identity includes the Archive fingerprint, containing Entry chain and leaf Entry index.
Short IDs and display paths may repeat. The source's exact verified bytes are in `source.raw`.

| Namespace | `data.record_type` | Original subresource indices |
| --- | --- | --- |
| `audio-bank` | `bank` | BKHD section ordinal |
| `audio-bank-section` | `bank-section` | Section ordinal |
| `audio-bank-media` | `bank-media` | DIDX section and triplet ordinals |
| `audio-bank-object` | `bank-object` | HIRC section and object ordinals |
| `audio-bank-hirc-action` | `bank-hirc-action` | HIRC section and object ordinals |
| `audio-bank-hirc-event` | `bank-hirc-event` | HIRC section and Event object ordinals |
| `audio-bank-hirc-event-action` | `bank-hirc-event-action` | HIRC section, Event object and action ordinals |
| `audio-bank-hirc-event-tail` | `bank-hirc-event-tail` | HIRC section and Event object ordinals |
| `audio-bank-hirc-sound` | `bank-hirc-sound` | HIRC section and Sound object ordinals |
| `audio-bank-hirc-container` | `bank-hirc-container` | HIRC section and container object ordinals |
| `audio-bank-hirc-container-child` | `bank-hirc-container-child` | HIRC section, container object and child ordinals |
| `audio-bank-music-segment` | `bank-music-segment` | HIRC section and segment object ordinals |
| `audio-bank-music-segment-child` | `bank-music-segment-child` | HIRC section, segment and child ordinals |
| `audio-bank-music-segment-stinger` | `bank-music-segment-stinger` | HIRC section, segment and stinger ordinals |
| `audio-bank-music-segment-marker` | `bank-music-segment-marker` | HIRC section, segment and marker ordinals |
| `audio-bank-music-track` | `bank-music-track` | HIRC section and track object ordinals |
| `audio-bank-music-track-source` | `bank-music-track-source` | HIRC section and track and source ordinals |
| `audio-bank-music-track-clip` | `bank-music-track-clip` | HIRC section and track and clip ordinals |
| `audio-bank-music-track-switch-member` | `bank-music-track-switch-member` | HIRC section and track and switch-member ordinals |
| `audio-bank-music-playlist` | `bank-music-playlist` | HIRC section and playlist object ordinals |
| `audio-bank-music-playlist-child` | `bank-music-playlist-child` | HIRC section and playlist and child ordinals |
| `audio-bank-music-playlist-stinger` | `bank-music-playlist-stinger` | HIRC section and playlist and stinger ordinals |
| `audio-bank-music-playlist-item` | `bank-music-playlist-item` | HIRC section and playlist and item ordinals |
| `audio-bank-music-switch` | `bank-music-switch` | HIRC section and switch object ordinals |
| `audio-bank-music-switch-child` | `bank-music-switch-child` | HIRC section and switch and child ordinals |
| `audio-bank-music-switch-stinger` | `bank-music-switch-stinger` | HIRC section and switch and stinger ordinals |
| `audio-bank-music-switch-group` | `bank-music-switch-group` | HIRC section and switch and group ordinals |
| `audio-bank-music-switch-node` | `bank-music-switch-node` | HIRC section and switch and physical node ordinals |
| `audio-bank-music-switch-target` | `bank-music-switch-target` | HIRC section and switch and traversal target ordinals |
| `audio-bank-hirc` | `bank-hirc` | HIRC section ordinal |
| `audio-bank-stid` | `bank-stid` | STID section ordinal |
| `audio-bank-name` | `bank-name` | STID section and name ordinals |
| `audio-bank-stid-tail` | `bank-stid-tail` | STID section ordinal |
| `audio-bank-failure` | `bank-failure` | Failure phase and section ordinal when known |

Each section row retains the original four tag bytes, including unknown and non-UTF-8 tags,
and its exact header and body ranges. A BKHD row retains its original version, bank ID,
language ID and reader version support. Ranges are half-open in decoded Entry coordinates.
Their starts, exclusive ends and payload ordinals use unsigned decimal strings.

Each original DIDX triplet retains its ID, authored DATA offset and length, and the
range of its declaration. Repeated media IDs are separate rows in original order. DATA
resolution reports `resolved`, `missing`, `ambiguous`, `unavailable` or `outside-data`.
A resolved row links the exact DATA section and retained byte range. A bounded prefix
read supplies the observed media class, optional RIFF length and prefetch decision.
The class can be `wem`, `midi`, `impulse` or `unknown`.

HIRC object rows retain original order, raw type bytes, IDs and raw object ranges.
The version-specific kind is derived when a unique supported BKHD version is available.
The HIRC section summary retains its declared object count and exact trailing range,
including an explicitly empty range. The current Audio capability lists coverage for
decoded object fields and [embedded WEM metadata](media-audio-metadata.md#wems-embedded-in-soundbanks)
separately from these physical envelopes.

Action rows from that same physical object and observed bank header carry the original
action type and target ID. Version 154 Play actions also carry the original bank ID
(including zero) and bank type. Other versions and non-Play actions have a null PlayBank
value, with receipts for the decision that applied. Each word has its own original
byte range. Both skipped-property counts are selectors for the PlayBank fields.
An unsupported Action layout retains the original body and reports its layout explicitly.
A malformed Action contributes the fields read before the failure to diagnostic output.

Event rows preserve the original action count and its byte range. Versions through 122 use a
little-endian unsigned 32-bit count. Versions 123 through 154 use a Wwise variable-length count.
Each action ID is emitted in original order with its exact word range, including repeated IDs.
A complete Event emits the exact residual body range. Diagnostic Event rows preserve readable
count fields and the actual count-prefix range. An unsupported layout retains its original body
and reports the version-dependent status explicitly.

Sound rows preserve the original plug-in ID, storage code, source ID, in-memory size and
source flag byte when present. Versions 151 through 154 also preserve the original cache ID.
Storage, playback and language flags retain the original words behind their interpretations.
Versions 150 through 153 expose the opaque range after the source fields. The optional
version 154 node retains its original parent ID, including zero, the original routing
counts and the word immediately before the parent. An admitted source plug-in block retains
its declared length and exact raw parameter range. A malformed Sound contributes its
successfully read fields and checked ranges to diagnostic output. An unsupported version
retains the original body and reports its layout explicitly.

Container rows preserve the version 154 Random, Sequence, Switch and Blend layout, original
parent ID and declared child count. The Random/Sequence row carries the original mode byte.
Each child ID is emitted in authored order with its own word range, including repeated IDs.
The checked node range, admitted child extent and suffix range refer to the retained bank.
A malformed suffix retains the admitted child rows. A child count that exceeds the original
body retains the count and leaves the child extent absent. Earlier undecoded versions retain
their physical object rows. Later unsupported versions report their layout explicitly.
The opt-in `original_hirc_containers_and_children_reconcile` test checks
the projected rows and retained child words against the slice reader.

Version 154 MusicSegment rows carry the original parent, counts, tempo, meter, grid period,
grid offset and duration. Floating-point values keep their original IEEE-754 words and a
finite JSON value when available. Separate ordered rows retain child track IDs, stinger
trigger and segment IDs, and marker IDs, positions and names. Stingers expose the checked
extent of the original scheduling bytes after the IDs. Marker names carry their byte range,
original NUL position and UTF-8 status. A malformed name keeps the earlier fully read rows
and the original segment remains available through the retained bank.

Version 154 MusicTrack rows carry the authored parent, list counts, raw track kind byte,
interpreted kind, and switch or state group when present. Separate rows preserve ordered
source occurrences, clips and switch members, including repeated IDs. Source rows retain
the original plug-in ID, storage code, cache ID, in-memory size and flags. Checked plug-in
parameter, automation and switch transition ranges refer to the retained bank. Clip times
carry their original IEEE-754 words and a finite JSON value when available. A malformed tail
keeps the preceding complete rows. Other layouts retain the physical HIRC object.

Version 154 MusicPlaylist rows retain the music node, ordered children and stingers, and
items in their original order. Each item preserves its original segment and item IDs,
following child count, play mode code, signed loop counts, weight, avoid-repeat count and
raw weight and shuffle selector bytes. Each item also carries its interpreted play mode,
selector booleans and zero-based nesting depth. The depth uses the prior original child counts.
Its `bnkHircPlaylistChildCounts` source use encodes the first count offset, count of preceding
words, byte stride, and byte width, in that order.
The transition rule count and checked complete-rule prefix locate the original
scheduling bytes in the retained bank. A truncated item leaves earlier complete rows
available and reports the malformed layout.

Version 154 MusicSwitch rows retain the music node, ordered children and stingers, raw
continue-playback and decision mode bytes, argument group IDs and type codes, and the checked
transition rule extent. Physical node rows expose the original key, packed link word, weight
and probability in file order, including unvisited nodes. Target rows follow the checked
nonzero-target traversal in order, with physical node indices, levels and ancestor indices.
Repeated visits to a physical node remain separate. A malformed path retains its preceding
checked targets and available physical nodes. The original tree extent refers to the retained
bank and can be shorter than its declared size for a truncated tree.

STID header rows retain the authored string type and name count. Each name row carries its
original ID and byte count, exact header and name ranges, original bytes and UTF-8 result.
Repeated IDs and empty names retain their ordinals. Valid UTF-8 preserves whitespace and NUL
characters. Invalid UTF-8 retains the validated prefix length and the invalid sequence length
when known. A successful name scan emits an exact residual range, which can be empty.
Diagnostic output for a truncated name retains the authored count and readable prefix.

The provider streams physical sections before reading the selected DIDX, HIRC and STID bodies.
The second checked pass uses the complete directory evidence to select a unique DATA
section. Repeated recognized sections produce diagnosed projection loss, and all original
occurrences are available in diagnostic output. An incomplete directory leaves unresolved
DATA selection unavailable. An out-of-range media declaration keeps its original triplet
and its diagnosis.

The field-origin companion binds exact uses to the actual physical Entry. A resolved media
class identifies the admitted signature and form used to classify it. A declared RIFF
length identifies its own consulted size word. The raw media range and its originating
DIDX declaration remain separate fields. Record status, directory metadata and payload
encryption retain their distinct meanings.
